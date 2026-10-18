//! Guarded first-Fiona default grant storage, not complete GiveSkill/CharacterEntity initialization.
//! The future runtime must place per-skill callbacks between log attempts and Submit, not after save.
use crate::{
    application::character_creation::EntityAuthority,
    domain::legacy_user_data::SqlDateTime2,
    error::Error,
    infrastructure::{
        character_initialization_logs as logs,
        character_lobby::UserKey,
        fiona_character_state::{self as state, StateError},
        fiona_initializer_profile::Profile,
    },
};
use sqlx::PgPool;
pub struct Grant {
    id: i64,
    name: zeroize::Zeroizing<Vec<u8>>,
    rank: i32,
    required_ap: i32,
    arguments: Vec<crate::domain::skill_runtime::Argument>,
    item_callback: bool,
    stat_callback: bool,
}
impl Grant {
    pub fn id(&self) -> i64 {
        self.id
    }
    pub fn key(&self) -> &[u8] {
        &self.name
    }
    pub fn rank(&self) -> i32 {
        self.rank
    }
    pub fn required_ap(&self) -> i32 {
        self.required_ap
    }
    pub fn arguments(&self) -> &[crate::domain::skill_runtime::Argument] {
        &self.arguments
    }
    pub fn item_callback(&self) -> bool {
        self.item_callback
    }
    pub fn stat_callback(&self) -> bool {
        self.stat_callback
    }
}
pub struct Prepared {
    source: [u8; 32],
    rows: Vec<Grant>,
}
impl Prepared {
    pub fn rows(&self) -> &[Grant] {
        &self.rows
    }
}
/// IDs use the same authority as CID/items. A failed attempt never rewinds its source counter.
pub fn prepare(
    profile: &Profile,
    authority: &mut EntityAuthority,
    mut utc: impl FnMut() -> SqlDateTime2,
) -> Result<Prepared, Error> {
    let mut rows = Vec::new();
    for skill in profile.skills() {
        let id = authority.allocate(utc())?.entity_time.ticks();
        rows.push(Grant {
            id,
            name: zeroize::Zeroizing::new(skill.key().to_vec()),
            rank: skill.rank(),
            required_ap: skill.required_ap(),
            arguments: skill.arguments().to_vec(),
            item_callback: skill.item_callback(),
            stat_callback: skill.stat_callback(),
        });
    }
    Ok(Prepared {
        source: *profile.source_state(),
        rows,
    })
}
/// A successful SQL write is not complete SkillRankChanged/CharacterEntity initialization.
#[must_use = "the creation runtime must apply the ordered cache/stat/observable callbacks before success"]
pub struct Outcome {
    pub inserted: usize,
    pub logs_written: usize,
    pub logs_failed: usize,
    /// Fresh stored rows require entity work; false does not certify complete initialization.
    pub runtime_callbacks_required: bool,
    pub source_operation_failed: bool,
}
#[derive(sqlx::FromRow)]
struct Stored {
    id: i64,
    name: Vec<u8>,
    rank: i32,
    locked: i16,
    ap: i32,
}
pub async fn save(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    prepared: &Prepared,
) -> Result<Outcome, StateError> {
    save_with_callbacks(pool, key, cid, prepared, |_| Ok(())).await
}
/// Execute each caller-owned runtime callback after that skill's independent log and
/// before the next skill or Submit. Hook completion does not certify complete entity effects.
/// Cancellation/failure can retain logs and in-memory effects without committed skill rows.
pub async fn save_with_callbacks(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    prepared: &Prepared,
    callback: impl FnMut(&Grant) -> Result<(), StateError>,
) -> Result<Outcome, StateError> {
    save_inner(pool, key, cid, prepared, callback, None).await
}
#[derive(Default)]
struct FreshContext {
    cache: crate::domain::skill_runtime::Cache,
    runtime: Option<crate::domain::fresh_skill_runtime::Detached>,
}
async fn save_inner(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    prepared: &Prepared,
    mut callback: impl FnMut(&Grant) -> Result<(), StateError>,
    mut fresh: Option<&mut FreshContext>,
) -> Result<Outcome, StateError> {
    tokio::time::timeout(std::time::Duration::from_secs(30),async{
  let mut tx=state::owned_first_fiona(pool,key,cid).await?;
  let binding:Option<Vec<u8>>=sqlx::query_scalar("SELECT source_sha256 FROM game_character_state_preservation WHERE singleton").fetch_one(&mut *tx).await?;
  if binding.as_deref()!=Some(prepared.source.as_slice()){return Err(StateError::Conflict)}
  let ready:bool=sqlx::query_scalar("SELECT initialized FROM game_skill_identity WHERE singleton FOR SHARE").fetch_one(&mut *tx).await?;if !ready{return Err(StateError::Uninitialized)}
  let slots:Vec<(i32,Option<Vec<u8>>)>=sqlx::query_as("SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=$1 ORDER BY row_id LIMIT 2 FOR UPDATE").bind(cid).fetch_all(&mut *tx).await?;
  if slots!=[(0,None)]{return Err(StateError::Conflict)}
  let old:Vec<Stored>=sqlx::query_as("SELECT row_id AS id,skill_utf16 AS name,rank,locked,current_ap AS ap FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 257 FOR UPDATE").bind(cid).fetch_all(&mut *tx).await?;
  if !old.is_empty(){
   if fresh.is_some(){return Err(StateError::Conflict)}
   if old.len()!=prepared.rows.len()||old.iter().any(|old|!prepared.rows.iter().any(|r|r.id==old.id&&r.name.as_slice()==old.name&&r.rank==old.rank&&old.locked==0&&old.ap==0)){return Err(StateError::Conflict)}
   tx.commit().await?;return Ok(Outcome{inserted:0,logs_written:0,logs_failed:0,runtime_callbacks_required:false,source_operation_failed:false});
  }
  if let Some(context)=fresh.as_deref_mut(){
   // The owner/revision/CID lock is already held. Existing AP is not independently
   // saved: these admitted callbacks leave it clean, just as LINQ SubmitChanges does.
   let row:Option<(i32,i32,i64)>=sqlx::query_as("SELECT ap,total_used_ap,last_ap_update_units300 FROM game_character_info WHERE id=$1 AND NOT dirty AND legacy_last_update IS NULL AND exp=0 AND play_time=0 AND selected_title=0").bind(cid).fetch_optional(&mut *tx).await?;
   let (ap,used,last)=row.ok_or(StateError::Conflict)?;
   if ap!=0||used!=0{return Err(StateError::Conflict)}
   let last=crate::domain::legacy_user_data::SqlDateTime::from_units300(last).and_then(|d|d.framework_date_time()).map_err(|_|StateError::Profile)?;
   context.runtime=Some(crate::domain::fresh_skill_runtime::Detached::new(ap,last,false).map_err(|_|StateError::Profile)?);
  }
  let mut written=0;let mut failed=0;let deadline=tokio::time::Instant::now()+std::time::Duration::from_secs(15);
  // GiveSkill attempts each log before the single original SubmitChanges. Keep independent commits.
  for row in &prepared.rows{if logs::give_skill(pool,&row.name,row.rank,deadline).await{written+=1}else{failed+=1};
   if let Some(context)=fresh.as_deref_mut(){
    let runtime=context.runtime.as_mut().ok_or(StateError::Profile)?;
    crate::domain::skill_multicast::after_grant(&mut context.cache,crate::domain::skill_multicast::Grant{key:row.key(),rank:row.rank(),required_ap:row.required_ap(),arguments:row.arguments(),item_skill:row.item_callback(),stat_skill:row.stat_callback()},runtime).map_err(|_|StateError::Profile)?;
   }else{callback(row)?}
  }
  for row in &prepared.rows{sqlx::query("INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked,current_ap) VALUES($1,$2,$3,$4,0,0)").bind(row.id).bind(cid).bind(row.name.as_slice()).bind(row.rank).execute(&mut *tx).await?;}
  tx.commit().await?;Ok(Outcome{inserted:prepared.rows.len(),logs_written:written,logs_failed:failed,runtime_callbacks_required:fresh.is_none()&&!prepared.rows.is_empty(),source_operation_failed:false})
 }).await.map_err(|_|StateError::Timeout)?
}
/// Completed only the detached skill phase, not title/status/ItemProxy/actor lifecycle.
/// The returned context has no network/observer/equipment attachment capabilities.
pub struct FreshOutcome {
    pub storage: Outcome,
    pub cache: crate::domain::skill_runtime::Cache,
    pub runtime: crate::domain::fresh_skill_runtime::Detached,
}
/// Fresh local context only. Refuses existing skill rows rather than silently reconstructing
/// an actor on replay. Profile construction already requires GuildScore=false; admission
/// additionally checks the actual Meditation key before any log or database effect.
/// Source timestamps remain untouched when AP is clean: no datetime round-trip UPDATE.
pub async fn save_fresh_detached(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    prepared: &Prepared,
) -> Result<FreshOutcome, StateError> {
    if prepared
        .rows
        .iter()
        .any(|r| !crate::domain::fresh_skill_runtime::Detached::admits(r.key()))
    {
        return Err(StateError::Profile);
    }
    let mut context = FreshContext::default();
    let storage = save_inner(pool, key, cid, prepared, |_| Ok(()), Some(&mut context)).await?;
    let runtime = context.runtime.ok_or(StateError::Profile)?;
    Ok(FreshOutcome {
        storage,
        cache: context.cache,
        runtime,
    })
}
/// Dispatch the recovered multicast after each independent log, inside the existing
/// pre-Submit boundary. SQL replay dispatches nothing. Runtime/AP persistence and
/// actor lifecycle remain caller obligations; Outcome retains that explicit debt.
pub async fn save_with_ordered_runtime<R>(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    prepared: &Prepared,
    cache: &mut crate::domain::skill_runtime::Cache,
    runtime: &mut R,
) -> Result<Outcome, StateError>
where
    R: crate::domain::skill_multicast::Runtime<Error = StateError>,
{
    use crate::domain::skill_multicast::{self, DispatchError, Grant};
    save_with_callbacks(pool, key, cid, prepared, |row| {
        skill_multicast::after_grant(
            cache,
            Grant {
                key: row.key(),
                rank: row.rank(),
                required_ap: row.required_ap(),
                arguments: row.arguments(),
                item_skill: row.item_callback(),
                stat_skill: row.stat_callback(),
            },
            runtime,
        )
        .map_err(|error| match error {
            DispatchError::Cache(_) => StateError::Profile,
            DispatchError::Runtime(error) => error,
        })
    })
    .await
}
/// GiveTitlePreset logs once per newly inserted title; exact replay does not repeat its events.
/// This wrapper must be used instead of the storage-only seed when composing creation.
pub async fn seed_titles(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    profile: &Profile,
) -> Result<Outcome, StateError> {
    tokio::time::timeout(std::time::Duration::from_secs(35), async {
        state::verify_source_binding(pool, profile.source_state()).await?;
        let ids = match state::seed_title_presets(pool, key, cid, profile.preset_ids()).await {
            Ok(ids) => ids,
            Err(StateError::Database(_) | StateError::Timeout) => {
                tracing::warn!("initial title operation failed;private details suppressed");
                return Ok(Outcome {
                    inserted: 0,
                    logs_written: 0,
                    logs_failed: 0,
                    runtime_callbacks_required: false,
                    source_operation_failed: true,
                });
            }
            Err(error) => return Err(error),
        };
        let mut written = 0;
        let mut failed = 0;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        for id in &ids {
            if logs::give_title(pool, *id, deadline).await {
                written += 1
            } else {
                failed += 1
            }
        }
        Ok(Outcome {
            inserted: ids.len(),
            logs_written: written,
            logs_failed: failed,
            runtime_callbacks_required: !ids.is_empty(),
            source_operation_failed: false,
        })
    })
    .await
    .map_err(|_| StateError::Timeout)?
}
