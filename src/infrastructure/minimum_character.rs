//! Operator-authorized native minimum:base stats/SP,default inventory/appearance/skills.
//! Optional legacy service actors are deferred;this is not complete legacy initialization.
use super::{
    character_lobby::UserKey, fiona_character_state as state, fiona_customization as customization,
    fiona_initializer_profile::Profile, fiona_managers, fiona_skill_grants, item_stats,
};
use crate::{
    application::{
        character_creation::EntityAuthority, fiona_inventory, inventory_delivery::DeliveryClock,
    },
    domain::{
        initial_inventory::InventoryError, item_colors::ColorRandom, item_time::ItemTimeError,
        legacy_user_data::SqlDateTime2,
    },
    error::Error,
};
use sqlx::PgPool;
#[derive(Debug, thiserror::Error)]
pub enum MinimumError {
    #[error(transparent)]
    State(#[from] state::StateError),
    #[error(transparent)]
    Managers(#[from] fiona_managers::ManagerError),
    #[error(transparent)]
    Customization(#[from] customization::CustomizationError),
    #[error(transparent)]
    Content(#[from] Error),
    #[error("native minimum database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("native minimum is partial,already initialized or outside its profile")]
    Conflict,
}
/// Only this module can issue completion after readback;no actor/selection capability implied.
pub struct Initialized {
    cid: i64,
    items: usize,
    skills: usize,
}
impl Initialized {
    pub fn cid(&self) -> i64 {
        self.cid
    }
    pub fn items(&self) -> usize {
        self.items
    }
    pub fn skills(&self) -> usize {
        self.skills
    }
}
pub struct Inputs<'a> {
    pub initializer: &'a Profile,
    pub appearance: customization::Appearance<'a>,
    pub stats: &'a item_stats::Package,
}
pub struct Clock<'a> {
    pub utc: &'a mut (dyn FnMut() -> Result<SqlDateTime2, Error> + Send),
    pub civil: &'a mut (dyn FnMut() -> Result<SqlDateTime2, Error> + Send),
}
/// CID is already committed by CharacterMaker;one authority supplies CID/items/skills.
/// Failure retains an initializing marker and requires lifecycle compensation,never replay.
pub async fn initialize(
    pool: &PgPool,
    owner: &UserKey<'_>,
    cid: i64,
    inputs: Inputs<'_>,
    authority: &mut EntityAuthority,
    random: &mut ColorRandom,
    clock: Clock<'_>,
) -> Result<Initialized, MinimumError> {
    let Inputs {
        initializer,
        appearance,
        stats,
    } = inputs;
    initializer.bind(appearance.catalog)?;
    appearance.inventory.bind(appearance.catalog)?;
    stats.bind(appearance.catalog)?;
    appearance.colors.bind(appearance.catalog)?;
    // Refuse malformed client appearance before any minimum initialization effect.
    appearance
        .profile
        .prepare(
            appearance.template,
            appearance.inventory,
            appearance.catalog,
            appearance.colors,
            appearance.arithmetic,
        )
        .map_err(customization::CustomizationError::Appearance)?;
    {
        let mut tx = state::owned_first_fiona(pool, owner, cid).await?;
        let existing: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM game_minimum_character_initialization WHERE cid=$1)",
        )
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
        if existing {
            return Err(MinimumError::Conflict);
        }
        sqlx::query("INSERT INTO game_minimum_character_initialization(cid,uid,profile_sha256)VALUES($1,$2,$3)").bind(cid).bind(owner.uid).bind(initializer.fingerprint().as_slice()).execute(&mut *tx).await?;
        tx.commit().await?;
    }
    let _base =
        fiona_managers::initialize(pool, owner, cid, initializer, appearance.catalog).await?;
    let inv_inputs = fiona_inventory::Inputs {
        class: initializer.character_class(),
        profile: appearance.inventory,
        catalog: appearance.catalog,
        stats,
        colors: appearance.colors,
    };
    let mut utc = || (clock.utc)().map_err(|_| ItemTimeError::Clock);
    let mut civil = || (clock.civil)().map_err(|_| InventoryError::Clock);
    let mut delivery_clock = DeliveryClock {
        authority,
        utc: &mut utc,
        civil: &mut civil,
    };
    let outcome = customization::customize(pool, owner, cid, initializer, appearance, |journals| {
        fiona_inventory::prepare_delivery(inv_inputs, cid, journals, random, &mut delivery_clock)
            .map_err(super::fiona_inventory_delivery::DeliveryError::Content)
    })
    .await?;
    let items = outcome.inventory.prepared.inventory().items().len();
    sqlx::query("INSERT INTO game_user_info(nexon_sn,last_login_cid,prologue_played)VALUES($1,$2,false)ON CONFLICT(nexon_sn)DO UPDATE SET last_login_cid=EXCLUDED.last_login_cid").bind(owner.uid).bind(cid).execute(pool).await?;

    let prepared = fiona_skill_grants::prepare_checked(initializer, authority, clock.utc)?;
    let skills = fiona_skill_grants::save_fresh_detached(pool, owner, cid, &prepared).await?;
    if skills.storage.source_operation_failed
        || skills.storage.runtime_callbacks_required
        || skills.storage.inserted != prepared.rows().len()
    {
        return Err(MinimumError::Conflict);
    }
    let mut tx = state::owned_first_fiona(pool, owner, cid).await?;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_items WHERE owner_id=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    let k: i64 = sqlx::query_scalar("SELECT count(*) FROM game_skills WHERE cid=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    let costumes: i64 = sqlx::query_scalar("SELECT count(*) FROM game_costumes WHERE cid=$1")
        .bind(cid)
        .fetch_one(&mut *tx)
        .await?;
    if n != items as i64
        || k != prepared.rows().len() as i64
        || costumes != outcome.appearance.len() as i64
    {
        return Err(MinimumError::Conflict);
    }
    let updated=sqlx::query("UPDATE game_minimum_character_initialization SET initialized=true WHERE cid=$1 AND uid=$2 AND profile_sha256=$3 AND NOT initialized").bind(cid).bind(owner.uid).bind(initializer.fingerprint().as_slice()).execute(&mut *tx).await?.rows_affected();
    if updated != 1 {
        return Err(MinimumError::Conflict);
    }
    tx.commit().await?;
    Ok(Initialized {
        cid,
        items,
        skills: prepared.rows().len(),
    })
}
