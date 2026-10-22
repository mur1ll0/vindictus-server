//! Insert-only lobby preservation and bounded real queries; original SQL Server is never opened.
use super::legacy_users::SourceBytes;
use crate::domain::{character_lobby::*, game_user::LegacyNameKey, legacy_user_data::*};
use sqlx::{PgConnection, PgPool};
use std::time::Duration;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum LobbyError {
    #[error("character-lobby database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("character-lobby input could not be read")]
    Io(#[from] std::io::Error),
    #[error("invalid private character-lobby export")]
    Export,
    #[error("character-lobby source snapshot conflicts with target")]
    Conflict,
    #[error("character-lobby operation deadline exceeded")]
    Timeout,
    #[error("character-lobby operation does not own the admitted user revision")]
    Ownership,
    #[error("existing characters require the recovered complete CharacterSummary dependencies")]
    Characters,
    #[error(transparent)]
    Value(#[from] LobbyValueError),
    #[error(transparent)]
    Date(#[from] LegacyUserDataError),
}
#[derive(serde::Serialize)]
pub struct ImportSummary {
    pub source_rows: [usize; 3],
    pub inserted: [usize; 3],
    pub existing: [usize; 3],
    pub verified: [usize; 3],
    pub game_authentication_available: bool,
}
async fn bounds(c: &mut PgConnection) -> Result<(), LobbyError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *c)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(c)
        .await?;
    Ok(())
}
async fn info(c: &mut PgConnection, uid: i32) -> Result<Option<UserInfo>, LobbyError> {
    let r:Option<(i32,Option<i64>,bool)>=sqlx::query_as("SELECT nexon_sn,last_login_cid,prologue_played FROM game_user_info WHERE nexon_sn=$1 FOR SHARE").bind(uid).fetch_optional(c).await?;
    Ok(
        r.map(|(nexon_sn, last_login_cid, prologue_played)| UserInfo {
            nexon_sn,
            last_login_cid,
            prologue_played,
        }),
    )
}
#[derive(sqlx::FromRow)]
struct ProductRow {
    id: i64,
    nexon_sn: i32,
    product_name_utf16: SourceBytes,
    argument_utf16: Option<SourceBytes>,
    expire_time_ticks: Option<i64>,
}
impl ProductRow {
    fn decode(self) -> Result<UserProduct, LobbyError> {
        Ok(UserProduct {
            id: self.id,
            nexon_sn: self.nexon_sn,
            product_name: LegacyBytes::from_bytes(self.product_name_utf16.into_bytes())?,
            argument: self
                .argument_utf16
                .map(|b| LegacyBytes::from_bytes(b.into_bytes()))
                .transpose()?,
            expire_time: self
                .expire_time_ticks
                .map(SqlDateTime2::from_ticks)
                .transpose()?,
        })
    }
}
async fn product(c: &mut PgConnection, id: i64) -> Result<Option<UserProduct>, LobbyError> {
    sqlx::query_as::<_,ProductRow>("SELECT id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks FROM game_user_products WHERE id=$1 FOR SHARE").bind(id).fetch_optional(c).await?.map(ProductRow::decode).transpose()
}
async fn care(c: &mut PgConnection, uid: i32) -> Result<Option<UserCareInfo>, LobbyError> {
    let r:Option<(i32,i16,i16,i64)>=sqlx::query_as("SELECT uid,kind,state,last_update_units300 FROM game_user_care_info WHERE uid=$1 FOR SHARE").bind(uid).fetch_optional(c).await?;
    r.map(|(uid, kind, state, last)| {
        Ok(UserCareInfo {
            uid,
            kind: u8::try_from(kind).map_err(|_| LobbyError::Export)?,
            state,
            last_update: SqlDateTime::from_units300(last)?,
        })
    })
    .transpose()
}
pub async fn preserve(
    pool: &PgPool,
    s: &LobbySnapshot,
    verify: bool,
) -> Result<ImportSummary, LobbyError> {
    s.validate()?;
    tokio::time::timeout(Duration::from_secs(10), async {
  let mut tx=pool.begin().await?;bounds(&mut tx).await?;let mut inserted=[0usize;3];
  for r in &s.user_info{
   if !verify{inserted[0]+=sqlx::query("INSERT INTO game_user_info(nexon_sn,last_login_cid,prologue_played) VALUES($1,$2,$3) ON CONFLICT DO NOTHING").bind(r.nexon_sn).bind(r.last_login_cid).bind(r.prologue_played).execute(&mut *tx).await?.rows_affected() as usize;}
   if info(&mut tx,r.nexon_sn).await?.as_ref()!=Some(r){return Err(LobbyError::Conflict)}
  }
  for r in &s.user_products{
   if !verify{inserted[1]+=sqlx::query("INSERT INTO game_user_products(id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks) VALUES($1,$2,$3,$4,$5) ON CONFLICT DO NOTHING").bind(r.id).bind(r.nexon_sn).bind(r.product_name.as_bytes()).bind(r.argument.as_ref().map(LegacyBytes::as_bytes)).bind(r.expire_time.map(|t|t.ticks())).execute(&mut *tx).await?.rows_affected() as usize;}
   if product(&mut tx,r.id).await?.as_ref()!=Some(r){return Err(LobbyError::Conflict)}
  }
  for r in &s.user_care_info{
   if !verify{inserted[2]+=sqlx::query("INSERT INTO game_user_care_info(uid,kind,state,last_update_units300) VALUES($1,$2,$3,$4) ON CONFLICT DO NOTHING").bind(r.uid).bind(i16::from(r.kind)).bind(r.state).bind(r.last_update.units300()).execute(&mut *tx).await?.rows_affected() as usize;}
   if care(&mut tx,r.uid).await?.as_ref()!=Some(r){return Err(LobbyError::Conflict)}
  }
  tx.commit().await?;let source_rows=[s.user_info.len(),s.user_products.len(),s.user_care_info.len()];
  Ok(ImportSummary{source_rows,inserted,existing:[source_rows[0]-inserted[0],source_rows[1]-inserted[1],source_rows[2]-inserted[2]],verified:source_rows,game_authentication_available:false})
 }).await.map_err(|_|LobbyError::Timeout)?
}
pub struct UserKey<'a> {
    pub uid: i32,
    pub name: &'a str,
    pub revision: i64,
}
#[derive(Clone, Copy)]
pub struct QueryConfig {
    pub max_free: i32,
    pub care_clock: Option<SqlDateTime>,
}
/// Recovered CharacterDataContext.ProloguePlayed: insert NULL CID or change only the flag.
/// The revision lock prevents an older connection from writing after another login.
pub async fn mark_prologue_played(pool: &PgPool, key: UserKey<'_>) -> Result<(), LobbyError> {
    let name = LegacyNameKey::parse(key.name).map_err(|_| LobbyError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut tx = pool.begin().await?;
        bounds(&mut tx).await?;
        let owner: Option<(String, i64)> = sqlx::query_as(
            "SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE",
        ).bind(key.uid).fetch_optional(&mut *tx).await?;
        if !owner.is_some_and(|(n, r)| n == name.as_str() && r == key.revision) {
            return Err(LobbyError::Ownership);
        }
        sqlx::query("INSERT INTO game_user_info(nexon_sn,last_login_cid,prologue_played) VALUES($1,NULL,true) ON CONFLICT(nexon_sn) DO UPDATE SET prologue_played=true WHERE NOT game_user_info.prologue_played")
            .bind(key.uid).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }).await.map_err(|_| LobbyError::Timeout)?
}
pub async fn query_empty(
    pool: &PgPool,
    key: UserKey<'_>,
    config: QueryConfig,
) -> Result<EmptyLobby, LobbyError> {
    let name = LegacyNameKey::parse(key.name).map_err(|_| LobbyError::Ownership)?;
    tokio::time::timeout(Duration::from_secs(10), async {
  let mut tx=pool.begin().await?;sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ").execute(&mut *tx).await?;bounds(&mut tx).await?;
  let owner:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
  if !owner.is_some_and(|(n,r)|n==name.as_str()&&r==key.revision){return Err(LobbyError::Ownership)}
  let u=info(&mut tx,key.uid).await?;
  let count:i64=sqlx::query_scalar("SELECT COUNT(*) FROM game_character_info WHERE uid=$1").bind(key.uid).fetch_one(&mut *tx).await?;
  if count!=0{return Err(LobbyError::Characters)}
  let products=sqlx::query_as::<_,ProductRow>("SELECT id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks FROM game_user_products WHERE nexon_sn=$1 ORDER BY id LIMIT 1001").bind(key.uid).fetch_all(&mut *tx).await?;
  if products.len()>MAX_ROWS{return Err(LobbyValueError::Bound.into())}
  let(mut purchased,mut premium)=(0i32,0i32);for r in products{let r=r.decode()?;match classify_product(&r.product_name)?{ProductKind::CharacterSlot=>purchased+=1,ProductKind::PremiumCharacterSlot=>premium+=1,ProductKind::Other=>{}}}
  let preset:i64=sqlx::query_scalar("SELECT COUNT(*) FROM game_character_info WHERE uid=$1 AND use_preset AND status<>1").bind(key.uid).fetch_one(&mut *tx).await?;
  if let Some(care_clock) = config.care_clock {sqlx::query("INSERT INTO game_user_care_info(uid,kind,state,last_update_units300) VALUES($1,1,2,$2) ON CONFLICT(uid) DO UPDATE SET kind=EXCLUDED.kind,state=EXCLUDED.state,last_update_units300=EXCLUDED.last_update_units300").bind(key.uid).bind(care_clock.units300()).execute(&mut *tx).await?;}
  tx.commit().await?;
  Ok(EmptyLobby{last_cid:u.as_ref().and_then(|r|r.last_login_cid).unwrap_or(0),prologue_played:u.is_some_and(|r|r.prologue_played),max_free:config.max_free,max_purchased:purchased,max_premium:premium,preset_used:i32::try_from(preset).map_err(|_|LobbyValueError::Bound)?})
 }).await.map_err(|_|LobbyError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), LobbyError> {
    sqlx::query("SELECT nexon_sn,last_login_cid,prologue_played FROM game_user_info LIMIT 0")
        .execute(pool)
        .await?;
    sqlx::query("SELECT id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks FROM game_user_products LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT uid,kind,state,last_update_units300 FROM game_user_care_info LIMIT 0")
        .execute(pool)
        .await?;
    Ok(())
}

/// Shared header fields for an admitted native list;does not fabricate an EmptyLobby.
pub(crate) async fn filled_header(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i32,
    config: QueryConfig,
) -> Result<(i32, i32, bool, i32), LobbyError> {
    let u = info(tx, uid).await?;
    let products=sqlx::query_as::<_,ProductRow>("SELECT id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks FROM game_user_products WHERE nexon_sn=$1 ORDER BY id LIMIT 1001").bind(uid).fetch_all(&mut **tx).await?;
    if products.len() > MAX_ROWS {
        return Err(LobbyValueError::Bound.into());
    }
    let (mut purchased, mut premium) = (0, 0);
    for r in products {
        let r = r.decode()?;
        match classify_product(&r.product_name)? {
            ProductKind::CharacterSlot => purchased += 1,
            ProductKind::PremiumCharacterSlot => premium += 1,
            ProductKind::Other => {}
        }
    }
    let preset: i64 = sqlx::query_scalar(
        "SELECT count(*)FROM game_character_info WHERE uid=$1 AND use_preset AND status<>1",
    )
    .bind(uid)
    .fetch_one(&mut **tx)
    .await?;
    if let Some(now) = config.care_clock {
        sqlx::query("INSERT INTO game_user_care_info(uid,kind,state,last_update_units300)VALUES($1,1,2,$2)ON CONFLICT(uid)DO UPDATE SET kind=EXCLUDED.kind,state=EXCLUDED.state,last_update_units300=EXCLUDED.last_update_units300").bind(uid).bind(now.units300()).execute(&mut **tx).await?;
    }
    Ok((
        purchased,
        premium,
        u.is_some_and(|r| r.prologue_played),
        i32::try_from(preset).map_err(|_| LobbyValueError::Bound)?,
    ))
}
