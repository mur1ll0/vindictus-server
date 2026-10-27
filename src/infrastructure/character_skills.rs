//! Skill/SPSkill source preservation; no grant or gameplay acceptance is implied.
use crate::{
    domain::legacy_user_data::LegacyBytes,
    infrastructure::{
        item_catalog,
        user_export::{PrivateText, integer, nullable},
    },
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{collections::HashSet, io::Read, time::Duration};
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum SkillError {
    #[error("skill snapshot is outside the recovered profile")]
    Export,
    #[error("skill preservation conflicts with stored state")]
    Conflict,
    #[error("skill database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("skill input could not be read")]
    Input(#[from] std::io::Error),
    #[error("skill operation exceeded its bounded deadline")]
    Timeout,
    #[error("skill identity has not been preserved")]
    Uninitialized,
    #[error("skill initialization owner or source profile is not admitted")]
    Ownership,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    #[serde(deserialize_with = "nullable")]
    sp_source_last_value: Option<PrivateText>,
    skills: Vec<RawSkill>,
    sp_slots: Vec<RawSlot>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkill {
    row_id: PrivateText,
    cid: PrivateText,
    skill_utf16: PrivateText,
    rank: i32,
    locked: u8,
    current_ap: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    row_id: i32,
    cid: PrivateText,
    slot_id: i32,
    #[serde(deserialize_with = "nullable")]
    skill_utf16: Option<PrivateText>,
}
struct Skill {
    id: i64,
    cid: i64,
    name: LegacyBytes<50, true>,
    rank: i32,
    locked: u8,
    ap: i32,
}
struct Slot {
    id: i32,
    cid: i64,
    slot: i32,
    name: Option<LegacyBytes<50, true>>,
}
pub struct Snapshot {
    last: Option<i32>,
    skills: Vec<Skill>,
    slots: Vec<Slot>,
}
fn id(v: PrivateText) -> Result<i64, SkillError> {
    integer(v).map_err(|_| SkillError::Export)
}
fn name(v: PrivateText) -> Result<LegacyBytes<50, true>, SkillError> {
    let bytes = item_catalog::opaque(Some(v))
        .map_err(|_| SkillError::Export)?
        .ok_or(SkillError::Export)?;
    LegacyBytes::from_bytes(bytes.as_bytes().to_vec()).map_err(|_| SkillError::Export)
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, SkillError> {
    let mut bytes = Zeroizing::new(Vec::new());
    input.take(16 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err(SkillError::Export);
    }
    let r: Export = serde_json::from_slice(&bytes).map_err(|_| SkillError::Export)?;
    if r.format_version != 1
        || r.source != "study_heroes.skill-state"
        || r.skills.len() > 100000
        || r.sp_slots.len() > 100000
    {
        return Err(SkillError::Export);
    }
    let last = r
        .sp_source_last_value
        .map(|v| i32::try_from(id(v)?).map_err(|_| SkillError::Export))
        .transpose()?;
    if last.is_some_and(|v| !(0..i32::MAX).contains(&v)) {
        return Err(SkillError::Export);
    }
    let mut seen = HashSet::new();
    let mut skills = Vec::with_capacity(r.skills.len());
    for r in r.skills {
        let row_id = id(r.row_id)?;
        if !seen.insert(row_id) {
            return Err(SkillError::Export);
        };
        skills.push(Skill {
            id: row_id,
            cid: id(r.cid)?,
            name: name(r.skill_utf16)?,
            rank: r.rank,
            locked: r.locked,
            ap: r.current_ap,
        });
    }
    let mut seen = HashSet::new();
    let mut slots = Vec::with_capacity(r.sp_slots.len());
    for r in r.sp_slots {
        if !seen.insert(r.row_id) || !last.is_some_and(|v| r.row_id <= v) {
            return Err(SkillError::Export);
        };
        slots.push(Slot {
            id: r.row_id,
            cid: id(r.cid)?,
            slot: r.slot_id,
            name: r.skill_utf16.map(name).transpose()?,
        });
    }
    Ok(Snapshot {
        last,
        skills,
        slots,
    })
}
#[derive(sqlx::FromRow)]
struct StoredSkill {
    row_id: i64,
    cid: i64,
    skill_utf16: Vec<u8>,
    rank: i32,
    locked: i16,
    current_ap: i32,
}
#[derive(sqlx::FromRow)]
struct StoredSlot {
    row_id: i32,
    cid: i64,
    slot_id: i32,
    skill_utf16: Option<Vec<u8>>,
}
#[derive(Serialize)]
pub struct Summary {
    pub skills: usize,
    pub sp_slots: usize,
    pub inserted: usize,
    pub verified: usize,
    pub identity_initialized: bool,
    pub character_creation_available: bool,
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, SkillError> {
    tokio::time::timeout(Duration::from_secs(30),async{
  let mut tx=pool.begin().await?;
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let state:(bool,Option<i32>)=sqlx::query_as("SELECT initialized,source_last_value FROM game_skill_identity WHERE singleton FOR UPDATE").fetch_one(&mut *tx).await?;
  if state.0&&state.1!=s.last{return Err(SkillError::Conflict)}
  sqlx::query("LOCK TABLE game_skills,game_sp_skills IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let mut inserted=0;
  for r in &s.skills {
   if !verify&&!state.0 {inserted+=sqlx::query("INSERT INTO game_skills(row_id,cid,skill_utf16,rank,locked,current_ap) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT DO NOTHING").bind(r.id).bind(r.cid).bind(r.name.as_bytes()).bind(r.rank).bind(i16::from(r.locked)).bind(r.ap).execute(&mut *tx).await?.rows_affected() as usize;}
   let stored:Option<StoredSkill>=sqlx::query_as("SELECT row_id,cid,skill_utf16,rank,locked,current_ap FROM game_skills WHERE row_id=$1").bind(r.id).fetch_optional(&mut *tx).await?;
   if !stored.is_some_and(|v|v.row_id==r.id&&v.cid==r.cid&&v.skill_utf16==r.name.as_bytes()&&v.rank==r.rank&&v.locked==i16::from(r.locked)&&v.current_ap==r.ap){return Err(SkillError::Conflict)}
  }
  for r in &s.slots {
   if !verify&&!state.0 {inserted+=sqlx::query("INSERT INTO game_sp_skills(row_id,cid,slot_id,skill_utf16) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(r.id).bind(r.cid).bind(r.slot).bind(r.name.as_ref().map(LegacyBytes::as_bytes)).execute(&mut *tx).await?.rows_affected() as usize;}
   let stored:Option<StoredSlot>=sqlx::query_as("SELECT row_id,cid,slot_id,skill_utf16 FROM game_sp_skills WHERE row_id=$1").bind(r.id).fetch_optional(&mut *tx).await?;
   if !stored.is_some_and(|v|v.row_id==r.id&&v.cid==r.cid&&v.slot_id==r.slot&&v.skill_utf16.as_deref()==r.name.as_ref().map(LegacyBytes::as_bytes)){return Err(SkillError::Conflict)}
  }
  if !verify&&!state.0 {
   let actual:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_skills),(SELECT COUNT(*) FROM game_sp_skills)").fetch_one(&mut *tx).await?;
   if actual!=(s.skills.len() as i64,s.slots.len() as i64){return Err(SkillError::Conflict)}
   let next=s.last.map_or(1,|v|v+1);
   sqlx::query(&format!("ALTER SEQUENCE game_sp_skill_row_id RESTART WITH {next}")).execute(&mut *tx).await?;
   sqlx::query("UPDATE game_skill_identity SET initialized=TRUE,source_last_value=$1 WHERE singleton").bind(s.last).execute(&mut *tx).await?;
  }
  tx.commit().await?;
  Ok(Summary{skills:s.skills.len(),sp_slots:s.slots.len(),inserted,verified:s.skills.len()+s.slots.len(),identity_initialized:state.0||!verify,character_creation_available:false})
 }).await.map_err(|_|SkillError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), SkillError> {
    sqlx::query("SELECT row_id,cid,skill_utf16,rank,locked,current_ap FROM game_skills LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT row_id,cid,slot_id,skill_utf16 FROM game_sp_skills LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT singleton,initialized,source_last_value FROM game_skill_identity LIMIT 0")
        .execute(pool)
        .await?;
    Ok(())
}
/// Source SkillManager.Initialize creates one NULL slot 0 when the collection is empty.
/// This boundary admits only a new normal Fiona belonging to a synthetic account.
pub async fn initialize_sp_slot(
    pool: &PgPool,
    key: &crate::infrastructure::character_lobby::UserKey<'_>,
    cid: i64,
) -> Result<usize, SkillError> {
    use crate::domain::game_user::LegacyNameKey;
    let owner = LegacyNameKey::parse(key.name).map_err(|_| SkillError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=pool.begin().await?;
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let ready:bool=sqlx::query_scalar("SELECT initialized FROM game_skill_identity WHERE singleton FOR SHARE").fetch_one(&mut *tx).await?;
  if !ready{return Err(SkillError::Uninitialized)}
  let user:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  let origin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
  if user!=Some((owner.as_str().to_owned(),key.revision))||!origin{return Err(SkillError::Ownership)}
  sqlx::query("LOCK TABLE game_character_info IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let owned:Option<i64>=sqlx::query_scalar("SELECT id FROM game_character_info WHERE id=$1 AND uid=$2 AND class BETWEEN 0 AND 9 AND level=1 AND NOT use_preset FOR UPDATE").bind(cid).bind(key.uid).fetch_optional(&mut *tx).await?;
  if owned.is_none(){return Err(SkillError::Ownership)}
  sqlx::query("LOCK TABLE game_sp_skills IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let stored:Vec<StoredSlot>=sqlx::query_as("SELECT row_id,cid,slot_id,skill_utf16 FROM game_sp_skills WHERE cid=$1 ORDER BY row_id LIMIT 1025 FOR UPDATE").bind(cid).fetch_all(&mut *tx).await?;
  let changed=if stored.is_empty(){
   sqlx::query("INSERT INTO game_sp_skills(row_id,cid,slot_id,skill_utf16) VALUES(nextval('game_sp_skill_row_id'),$1,0,NULL)").bind(cid).execute(&mut *tx).await?;1
  }else if stored.len()==1&&stored[0].slot_id==0&&stored[0].skill_utf16.is_none(){0}else{return Err(SkillError::Conflict)};
  tx.commit().await?;Ok(changed)
 }).await.map_err(|_|SkillError::Timeout)?
}
