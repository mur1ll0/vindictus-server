//! Restricted new-Fiona base-stat write. Not the complete CharacterEntity initializer.
use crate::{
    domain::{character_base_stats::BaseStats, game_user::LegacyNameKey},
    infrastructure::character_lobby::UserKey,
};
use sqlx::PgPool;
use std::time::Duration;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum BaseStatError {
    #[error("base stat database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("base stat writer does not own the synthetic Fiona revision")]
    Ownership,
    #[error("base stats conflict with stored state; nothing repaired")]
    Conflict,
    #[error("base stat operation timed out; verify original attempt before retry")]
    Timeout,
}
#[derive(sqlx::FromRow)]
struct Stored {
    dirty: bool,
    str: i32,
    dex: i32,
    int: i32,
    will: i32,
    luck: i32,
    hp: i32,
    stamina: i32,
}
impl Stored {
    fn values(&self) -> [i32; 7] {
        [
            self.str,
            self.dex,
            self.int,
            self.will,
            self.luck,
            self.hp,
            self.stamina,
        ]
    }
}
/// Changes only a pristine base row. Exact replay is allowed; other states are not repaired.
pub async fn initialize(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    stats: &BaseStats,
) -> Result<usize, BaseStatError> {
    let owner = LegacyNameKey::parse(key.name).map_err(|_| BaseStatError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(15),async{
  let mut tx=pool.begin().await?;
  sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
  sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
  let user:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  let origin:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
  if user!=Some((owner.as_str().to_owned(),key.revision))||!origin{return Err(BaseStatError::Ownership)}
  sqlx::query("LOCK TABLE game_character_info IN ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
  let stored:Option<Stored>=sqlx::query_as("SELECT dirty,str,dex,int,will,luck,hp,stamina FROM game_character_info WHERE id=$1 AND uid=$2 AND class=1 AND level=1 AND NOT use_preset FOR UPDATE").bind(cid).bind(key.uid).fetch_optional(&mut *tx).await?;
  let row=stored.ok_or(BaseStatError::Ownership)?;
  if !row.dirty&&row.values()==*stats.values(){tx.commit().await?;return Ok(0)}
  if !row.dirty||row.values()!=[0;7]{return Err(BaseStatError::Conflict)}
  let v=stats.values();
  sqlx::query("UPDATE game_character_info SET dirty=false,str=$2,dex=$3,int=$4,will=$5,luck=$6,hp=$7,stamina=$8 WHERE id=$1").bind(cid).bind(v[0]).bind(v[1]).bind(v[2]).bind(v[3]).bind(v[4]).bind(v[5]).bind(v[6]).execute(&mut *tx).await?;
  tx.commit().await?;Ok(1)
 }).await.map_err(|_|BaseStatError::Timeout)?
}
