//! Scoped writers required by normal creation; not a generic AddCharacterLedger port.
//! Existing ledger history is not read by creation and remains a preservation dependency.
use super::character_lobby::UserKey;
use crate::domain::{
    character_ledger::{LedgerPartition, partition},
    game_user::LegacyNameKey,
    legacy_user_data::SqlDateTime,
};
use sqlx::PgPool;
use thiserror::Error;
use zeroize::Zeroizing;
#[derive(Debug, Error)]
pub enum CreationLedgerError {
    #[error("creation ledger database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("creation ledger no longer owns a synthetic admitted revision")]
    Ownership,
    #[error("creation ledger is outside the recovered bounded profile")]
    Profile,
    #[error("creation ledger deadline exceeded")]
    Timeout,
}
fn utf16(s: &str, max: usize) -> Result<Zeroizing<Vec<u8>>, CreationLedgerError> {
    if s.encode_utf16().take(max + 1).count() > max {
        return Err(CreationLedgerError::Profile);
    }
    Ok(Zeroizing::new(
        s.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ))
}
pub(crate) async fn changed_name_check(
    pool: &PgPool,
    key: &UserKey<'_>,
    name: &str,
    period: i32,
) -> Result<(), CreationLedgerError> {
    let event = Zeroizing::new(format!("ChangedNameCheck-{}", key.name));
    // Called only after a successful history check: its original COUNT result is zero.
    let target = Zeroizing::new(format!("I:{name},I:{period},O:0"));
    append(pool, key, &event, &target).await
}
pub async fn dormant_create(
    pool: &PgPool,
    key: &UserKey<'_>,
    use_preset: bool,
) -> Result<(), CreationLedgerError> {
    let target = if use_preset {
        "UsePreset: True"
    } else {
        "UsePreset: False"
    };
    append(pool, key, "Dormant - CreateCharacter", target).await
}
async fn append(
    pool: &PgPool,
    key: &UserKey<'_>,
    event: &str,
    target: &str,
) -> Result<(), CreationLedgerError> {
    let owner = LegacyNameKey::parse(key.name).map_err(|_| CreationLedgerError::Ownership)?;
    let event = utf16(event, 50)?;
    let target = utf16(target, 256)?;
    tokio::time::timeout(std::time::Duration::from_secs(10),async {
        let mut tx=pool.begin().await?;
        sqlx::query("SET LOCAL lock_timeout='2s'").execute(&mut *tx).await?;
        sqlx::query("SET LOCAL statement_timeout='5s'").execute(&mut *tx).await?;
        let actual:Option<(String,i64)>=sqlx::query_as("SELECT name_key,local_revision FROM game_users WHERE id=$1 FOR SHARE").bind(key.uid).fetch_optional(&mut *tx).await?;
        if !actual.is_some_and(|(n,r)|n==owner.as_str()&&r==key.revision) {return Err(CreationLedgerError::Ownership)}
        let local:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_lab_account_origins WHERE uid=$1 AND name_key=$2)").bind(key.uid).bind(owner.as_str()).fetch_one(&mut *tx).await?;
        if !local {return Err(CreationLedgerError::Ownership)}
        // Routing and the table's default Time are separate samples in the source procedure.
        let units:i64=sqlx::query_scalar("SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT").fetch_one(&mut *tx).await?;
        let civil=SqlDateTime::from_units300(units).map_err(|_|CreationLedgerError::Profile)?;
        let statement=match partition(civil) {
            LedgerPartition::One=>"INSERT INTO game_character_ledger01(journal_id,event_utf16,target_utf16,value) VALUES(0,$1,$2,0)",
            LedgerPartition::Two=>"INSERT INTO game_character_ledger02(journal_id,event_utf16,target_utf16,value) VALUES(0,$1,$2,0)",
        };
        sqlx::query(statement).bind(event.as_slice()).bind(target.as_slice()).execute(&mut *tx).await?;
        tx.commit().await?;Ok(())
    }).await.map_err(|_|CreationLedgerError::Timeout)?
}
pub async fn check_ready(pool: &PgPool) -> Result<(), CreationLedgerError> {
    for q in [
        "SELECT journal_id,event_utf16,target_utf16,value,time_units300,legacy_last_update FROM game_character_ledger01 LIMIT 0",
        "SELECT journal_id,event_utf16,target_utf16,value,time_units300,legacy_last_update FROM game_character_ledger02 LIMIT 0",
    ] {
        sqlx::query(q).execute(pool).await?;
    }
    Ok(())
}
