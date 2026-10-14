//! CharacterMaker normal Fiona branch. Base creation is NOT completed character initialization.
use crate::{
    domain::{
        character_allocator::make_entity_id,
        character_creation::*,
        legacy_user_data::{LegacyUserDataError, SqlDateTime2},
    },
    infrastructure::{
        character_creation_admission::{self as admission, CreationAdmissionError},
        character_creation_core::{
            self as core, CreationCoreError, CreationCoreRequest, CreationCoreResult,
        },
        character_creation_effects::{self as effects, CreationEffectsError},
        character_creation_ledger,
        character_lobby::UserKey,
        character_name_check::{self as names, NameCheckError, NameCheckResult},
        character_name_proof::NameProof,
    },
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum MakerError {
    #[error(transparent)]
    Admission(#[from] CreationAdmissionError),
    #[error(transparent)]
    Name(#[from] NameCheckError),
    #[error(transparent)]
    Core(#[from] CreationCoreError),
    #[error(transparent)]
    Effects(#[from] CreationEffectsError),
    #[error(transparent)]
    Date(#[from] LegacyUserDataError),
}
/// Explicit Service.ID and process-local counter, distinct from FeatureMatrix.ServerCode.
/// Caller must serialize access to this authority; restarting follows source counter lifetime.
pub struct EntityAuthority {
    service_id: i32,
    previous: i64,
}
impl EntityAuthority {
    pub fn new(service_id: i32) -> Self {
        Self {
            service_id,
            previous: 0,
        }
    }
    pub fn counter(&self) -> i64 {
        self.previous
    }
}
pub struct MakerSettings {
    pub stopped: bool,
    pub max_free: i32,
    pub server_code: i32,
}
/// No Debug/Serialize: owner and candidate are private input.
pub struct NormalFionaRequest<'a> {
    pub owner: UserKey<'a>,
    pub proof: &'a NameProof,
    pub name: &'a str,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BaseOutcome {
    Created {
        cid: i64,
        serial: i32,
    },
    Rejected {
        result: CreateResult,
        cid: i64,
        serial: i32,
    },
}
fn key<'a>(k: &UserKey<'a>) -> UserKey<'a> {
    UserKey {
        uid: k.uid,
        name: k.name,
        revision: k.revision,
    }
}
pub fn name_result(result: NameCheckResult) -> CreateResult {
    match result {
        NameCheckResult::ChangedCharacterName => CreateResult::ChangedCharacterName,
        NameCheckResult::DuplicatedName => CreateResult::DuplicatedName,
        NameCheckResult::ForbiddenName => CreateResult::ForbiddenName,
        NameCheckResult::InvalidName => CreateResult::InvalidName,
        NameCheckResult::ReservedName => CreateResult::ReservedName,
        _ => CreateResult::Unknown,
    }
}
/// Not exposed to TCP. The processor still needs completion/failure lifecycle and initializer.
/// Admission/name audit commits precede base SQL, as in the source; no transaction erases them.
pub async fn make_normal_fiona_base<F>(
    pool: &PgPool,
    r: NormalFionaRequest<'_>,
    settings: &MakerSettings,
    authority: &mut EntityAuthority,
    mut utc: F,
) -> Result<BaseOutcome, MakerError>
where
    F: FnMut() -> Result<SqlDateTime2, CreationAdmissionError>,
{
    let reject = |result, serial| BaseOutcome::Rejected {
        result,
        cid: -1,
        serial,
    };
    if settings.stopped {
        return Ok(reject(CreateResult::CreateCharacterStopped, -1));
    }
    if admission::slots(pool, key(&r.owner))
        .await?
        .at_limit(settings.max_free)
    {
        return Ok(reject(CreateResult::CharacterLimit, -1));
    }
    let result = names::check_for_creation(pool, key(&r.owner), r.proof, Some(r.name)).await?;
    if result != NameCheckResult::Success {
        return Ok(reject(name_result(result), -1));
    }
    // Explicit finite-profile refusal for old matching deleted names: no arbitrary cleanup CID.
    effects::require_empty_cleanup_candidate(pool, &r.owner, r.proof).await?;
    let sn = admission::serial_with_clock(pool, key(&r.owner), settings.server_code, &mut utc)
        .await?
        .serial;
    let cid = make_entity_id(&mut authority.previous, authority.service_id, utc()?)?
        .entity_time
        .ticks();
    match core::create_base(
        pool,
        key(&r.owner),
        r.proof,
        CreationCoreRequest {
            cid,
            character_sn: sn,
            name: r.name,
            class: 1,
            use_preset: false,
        },
    )
    .await?
    {
        CreationCoreResult::Created => Ok(BaseOutcome::Created { cid, serial: sn }),
        CreationCoreResult::ExistingCid | CreationCoreResult::DuplicateNameOrSn => {
            Ok(reject(CreateResult::DuplicatedName, sn))
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DormantOutcome {
    Ready { ledger_written: bool },
    InitCharacterFailed,
}
/// Source try covers both operations; a persisted connection tolerates even a ledger exception.
/// Call only when effective DormantAccount=true. Feature binding remains the processor's job.
pub async fn prepare_dormant(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    utc: SqlDateTime2,
) -> Result<DormantOutcome, MakerError> {
    let inserted = match dormant_parameter(utc) {
        Ok(date) => effects::insert_dormant_connection(pool, owner, cid, date)
            .await
            .is_ok(),
        Err(_) => false,
    };
    if inserted
        && character_creation_ledger::dormant_create(pool, owner, false)
            .await
            .is_ok()
    {
        return Ok(DormantOutcome::Ready {
            ledger_written: true,
        });
    }
    if effects::has_connection(pool, owner, cid).await? {
        Ok(DormantOutcome::Ready {
            ledger_written: false,
        })
    } else {
        Ok(DormantOutcome::InitCharacterFailed)
    }
}
