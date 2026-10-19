//! Recovered IsTransCharacter empty-reference branch only; transfer workflows remain unsupported.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use std::{io::Read, time::Duration};
#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("invalid bounded empty transfer reference")]
    Input,
    #[error("transfer guard database operation failed")]
    Database(#[from] sqlx::Error),
    #[error("transfer reference missing,changed or outside empty branch")]
    Reference,
    #[error("transfer guard deadline exceeded;reconcile before retry")]
    Timeout,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    rows: usize,
    columns: usize,
    schema_sha256: String,
    procedure_sha256: String,
}
pub struct Snapshot {
    hash: [u8; 32],
    schema: String,
    procedure: String,
}
pub fn read_snapshot(input: impl Read) -> Result<Snapshot, TransferError> {
    let mut bytes = Vec::new();
    input
        .take(8193)
        .read_to_end(&mut bytes)
        .map_err(|_| TransferError::Input)?;
    if bytes.len() > 8192 {
        return Err(TransferError::Input);
    }
    let e: Export = serde_json::from_slice(&bytes).map_err(|_| TransferError::Input)?;
    let hash = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    };
    if e.format_version != 1
        || e.source != "study_heroes.character-transfer-empty"
        || e.rows != 0
        || e.columns != 8
        || !hash(&e.schema_sha256)
        || e.procedure_sha256 != "6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa"
    {
        return Err(TransferError::Input);
    }
    Ok(Snapshot {
        hash: Sha256::digest(&bytes).into(),
        schema: e.schema_sha256,
        procedure: e.procedure_sha256,
    })
}
#[derive(Serialize)]
pub struct Summary {
    pub rows: usize,
    pub columns: usize,
    pub initialized: bool,
    pub replay: bool,
    pub character_creation_available: bool,
}
async fn bounds(tx: &mut Transaction<'_, Postgres>) -> Result<(), TransferError> {
    sqlx::query("SET LOCAL lock_timeout='2s'")
        .execute(&mut **tx)
        .await?;
    sqlx::query("SET LOCAL statement_timeout='5s'")
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn preserve(pool: &PgPool, s: &Snapshot, verify: bool) -> Result<Summary, TransferError> {
    tokio::time::timeout(Duration::from_secs(15),async{
 let mut tx=pool.begin().await?;bounds(&mut tx).await?;
 sqlx::query("LOCK TABLE game_character_transfer_guard,game_character_transfer_info IN SHARE ROW EXCLUSIVE MODE").execute(&mut *tx).await?;
 let row:Option<(Vec<u8>,String,String)>=sqlx::query_as("SELECT source_sha256,schema_sha256,procedure_sha256 FROM game_character_transfer_guard WHERE singleton").fetch_optional(&mut *tx).await?;
 let replay=row.is_some();
 if let Some((hash,schema,procedure))=row{
  if hash!=s.hash||schema!=s.schema||procedure!=s.procedure{return Err(TransferError::Reference)}
 }else{
  if verify{return Err(TransferError::Reference)}
  sqlx::query("INSERT INTO game_character_transfer_guard(singleton,source_sha256,schema_sha256,procedure_sha256)VALUES(true,$1,$2,$3)").bind(s.hash.as_slice()).bind(&s.schema).bind(&s.procedure).execute(&mut *tx).await?;
 }
 require_empty(&mut tx).await?;
 tx.commit().await?;Ok(Summary{rows:0,columns:8,initialized:true,replay,character_creation_available:false})
 }).await.map_err(|_|TransferError::Timeout)?
}
/// Initialized full empty table,not a missing-row fallback. SHARE prevents inserts
/// until the caller has consumed the guard. Stronger than source NOLOCK.
pub async fn require_empty(tx: &mut Transaction<'_, Postgres>) -> Result<(), TransferError> {
    bounds(tx).await?;
    sqlx::query(
        "LOCK TABLE game_character_transfer_guard,game_character_transfer_info IN SHARE MODE",
    )
    .execute(&mut **tx)
    .await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_character_transfer_guard WHERE singleton)",
    )
    .fetch_one(&mut **tx)
    .await?;
    let occupied: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM game_character_transfer_info)")
            .fetch_one(&mut **tx)
            .await?;
    if !initialized || occupied {
        return Err(TransferError::Reference);
    }
    Ok(())
}
pub async fn check_ready(pool: &PgPool) -> Result<(), TransferError> {
    sqlx::query("SELECT uid,old_name_utf16,old_cid,old_sn,new_name_utf16,new_cid,new_sn,change_name_utf16 FROM game_character_transfer_info LIMIT 0").execute(pool).await?;
    sqlx::query("SELECT singleton,source_sha256,schema_sha256,procedure_sha256 FROM game_character_transfer_guard LIMIT 0").execute(pool).await?;
    Ok(())
}
