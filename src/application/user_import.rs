//! Validate the whole batch before acquiring database locks.
use crate::{
    domain::legacy_user_data::LegacyUserSnapshot,
    error::Error,
    infrastructure::{legacy_users, user_export::MAX_ROWS},
};
use sqlx::PgPool;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportMode {
    Apply,
    VerifyOnly,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportSummary {
    pub source_rows: usize,
    pub inserted: usize,
    pub existing: usize,
    pub verified: usize,
}
pub async fn import_users(
    pool: &PgPool,
    snapshots: &[LegacyUserSnapshot],
    mode: ImportMode,
) -> Result<ImportSummary, Error> {
    if snapshots.is_empty() || snapshots.len() > MAX_ROWS {
        return Err(Error::UserExport);
    }
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for snapshot in snapshots {
        let key = snapshot.validate()?;
        if !ids.insert(snapshot.user.id) || !names.insert(key.as_str().to_owned()) {
            return Err(Error::UserExport);
        }
    }
    let mut ordered: Vec<_> = snapshots.iter().collect();
    ordered.sort_unstable_by_key(|snapshot| snapshot.user.id);
    let inserted = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        legacy_users::execute_batch(pool, &ordered, mode == ImportMode::VerifyOnly),
    )
    .await
    .map_err(|_| Error::UserImportDeadline)??;
    Ok(ImportSummary {
        source_rows: snapshots.len(),
        inserted,
        existing: snapshots.len() - inserted,
        verified: snapshots.len(),
    })
}
