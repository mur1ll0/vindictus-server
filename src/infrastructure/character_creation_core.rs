//! Recovered base SQL creation, restricted to the measured normal Fiona lab path.
//! Not wired to the client: caller slots/logging/default items/appearance/Summary remain required.
use super::{character_lobby::UserKey, character_name_proof::NameProof, legacy_users::SourceBytes};
use crate::domain::{game_user::LegacyNameKey, legacy_user_data::SqlDateTime};
use sqlx::PgPool;
use std::time::Duration;
use thiserror::Error;
use zeroize::Zeroizing;

#[derive(Debug, Error)]
pub enum CreationCoreError {
    #[error("base character creation database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("base character creation does not own a synthetic admitted revision")]
    Ownership,
    #[error("base character creation is outside the recovered minimum profile")]
    Profile,
    #[error("base character creation deadline exceeded")]
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreationCoreResult {
    Created,
    ExistingCid,
    DuplicateNameOrSn,
}
/// The identifiers must come from the recovered allocator and collision loop in the caller.
/// No Debug/Serialize: private names are not diagnostic fields.
pub struct CreationCoreRequest<'a> {
    pub cid: i64,
    pub character_sn: i32,
    pub name: &'a str,
    pub class: u8,
    pub use_preset: bool,
}

pub async fn create_base(
    pool: &PgPool,
    key: UserKey<'_>,
    proof: &NameProof,
    request: CreationCoreRequest<'_>,
) -> Result<CreationCoreResult, CreationCoreError> {
    tokio::time::timeout(
        Duration::from_secs(10),
        create_owned(pool, key, proof, request),
    )
    .await
    .map_err(|_| CreationCoreError::Timeout)?
}
async fn create_owned(
    pool: &PgPool,
    key: UserKey<'_>,
    proof: &NameProof,
    r: CreationCoreRequest<'_>,
) -> Result<CreationCoreResult, CreationCoreError> {
    // BaseCharacter of the CustomizeCharacter switch; the caller checked its contents.
    if r.class > 9
        || r.use_preset
        || !proof
            .valid_request(Some(r.name))
            .map_err(|_| CreationCoreError::Profile)?
    {
        return Err(CreationCoreError::Profile);
    }
    let owner = LegacyNameKey::parse(key.name).map_err(|_| CreationCoreError::Ownership)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut *tx)
        .await?;
    let actual: Option<(String, i64)> =
        sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE")
            .bind(key.uid)
            .fetch_optional(&mut *tx)
            .await?;
    if !actual.is_some_and(|(name, revision)| name == owner.as_str() && revision == key.revision) {
        return Err(CreationCoreError::Ownership);
    }
    let synthetic: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)",
    )
    .bind(key.uid)
    .bind(owner.as_str())
    .fetch_one(&mut *tx)
    .await?;
    if !synthetic {
        return Err(CreationCoreError::Ownership);
    }
    // Serialize the name check with insertion. There is no source unique name/SN key.
    // This is a target concurrency guard, not a claim of SQL Server NOLOCK equivalence.
    sqlx::query("LOCK TABLE game_character_info IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut *tx)
        .await?;
    let cid_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_info WHERE id=$1)")
            .bind(r.cid)
            .fetch_one(&mut *tx)
            .await?;
    if cid_exists {
        tx.rollback().await?;
        return Ok(CreationCoreResult::ExistingCid);
    }
    let names: Vec<SourceBytes> = sqlx::query_scalar("SELECT name_utf16 FROM game_character_info WHERE status<>1 AND name_utf16 IS NOT NULL LIMIT 1001")
        .fetch_all(&mut *tx).await?;
    if names.len() > 1000 {
        return Err(CreationCoreError::Profile);
    }
    for name in names {
        let bytes = Zeroizing::new(name.into_bytes());
        if proof.equals(r.name, &bytes) {
            tx.rollback().await?;
            return Ok(CreationCoreResult::DuplicateNameOrSn);
        }
    }
    let sn_exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_info WHERE uid=$1 AND character_sn=$2 AND status<>1)")
        .bind(key.uid).bind(r.character_sn).fetch_one(&mut *tx).await?;
    if sn_exists {
        tx.rollback().await?;
        return Ok(CreationCoreResult::DuplicateNameOrSn);
    }
    // This phase implements the source's measured FreeItem-disabled branch only.
    // The enabled queue branch must be implemented before permitting another profile.
    sqlx::query("LOCK TABLE game_my_settings IN SHARE MODE")
        .execute(&mut *tx)
        .await?;
    let mut settings: Vec<(SourceBytes, SourceBytes)> =
        sqlx::query_as("SELECT code_bytes,value_bytes FROM game_my_settings LIMIT 2")
            .fetch_all(&mut *tx)
            .await?;
    if settings.len() != 1 {
        return Err(CreationCoreError::Profile);
    }
    let (code, value) = settings.pop().ok_or(CreationCoreError::Profile)?;
    let code = Zeroizing::new(code.into_bytes());
    let value = Zeroizing::new(value.into_bytes());
    // Exact measured setting, not a general SQL collation/char(1) adapter.
    if code.as_slice() != b"FreeItem" || value.as_slice() != b"0" {
        return Err(CreationCoreError::Profile);
    }
    // Source GETUTCDATE is SQL datetime, including the conversion to datetime2(7).
    // The SELECT-only 300-phase oracle confirms millisecond conversion, not rational 1/300 ticks.
    let utc_units: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
    )
    .fetch_one(&mut *tx)
    .await?;
    let create_ticks = SqlDateTime::from_units300(utc_units)
        .and_then(SqlDateTime::framework_date_time)
        .map_err(|_| CreationCoreError::Profile)?
        .ticks();
    let name = Zeroizing::new(
        r.name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    sqlx::query("INSERT INTO game_characters(id) VALUES($1)")
        .bind(r.cid)
        .execute(&mut *tx)
        .await?;
    sqlx::query("INSERT INTO game_character_info(id,uid,character_sn,name_utf16,class,level,exp,str,dex,int,will,luck,hp,stamina,dirty,play_time,selected_title,ap,total_used_ap,last_ap_update_units300,permission,status,quote_utf16,delete_time_units300,region_code,is_connected,legacy_last_update,create_time_ticks,free_match_win_count,free_match_lose_count,use_preset,is_premium,free_title_name_utf16,today_play_time,channel,order_num,map_state_info) VALUES($1,$2,$3,$4,$5,1,0,0,0,0,0,0,0,0,true,0,0,0,0,$6,3,0,NULL,NULL,0,false,NULL,$7,0,0,false,false,NULL,NULL,NULL,NULL,0)")
        .bind(r.cid).bind(key.uid).bind(r.character_sn).bind(name.as_slice()).bind(i16::from(r.class)).bind(utc_units).bind(create_ticks)
        .execute(&mut *tx).await?;
    for statement in [
        "INSERT INTO game_item_collections(owner_id) VALUES($1)",
        "INSERT INTO game_quest_collections(owner_id) VALUES($1)",
        "INSERT INTO game_mailboxes(owner_id) VALUES($1)",
        "INSERT INTO game_story_entries(cid) VALUES($1)",
    ] {
        sqlx::query(statement).bind(r.cid).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(CreationCoreResult::Created)
}
