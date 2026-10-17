//! Guarded internal delivery coordinator. Not a complete ItemOwner or TCP Create.
use super::{
    character_lobby::UserKey,
    fiona_character_state::{self, StateError},
    fiona_initializer_profile::Profile,
    initial_inventory::{self, InventoryWriteError},
    item_event_logs::{self, BatchOutcome},
    item_journals,
};
use crate::{
    application::inventory_delivery::{Journals, Prepared},
    domain::{
        initial_inventory::InventoryError,
        item_logs::{Counts, Ledger},
    },
};
use sqlx::PgPool;
use thiserror::Error;
#[derive(Debug, Error)]
pub enum DeliveryError {
    #[error(transparent)]
    Admission(#[from] StateError),
    #[error(transparent)]
    Storage(#[from] InventoryWriteError),
    #[error(transparent)]
    Preparation(#[from] InventoryError),
    #[error("delivery profile is outside the admitted source path")]
    Profile,
    #[error(transparent)]
    Content(#[from] crate::error::Error),
    #[error("inventory already exists; reconcile the original delivery before retrying")]
    Reconcile,
    #[error("shared inventory state must be loaded before character inventory delivery")]
    SharedState,
}
pub struct Outcome {
    pub prepared: Prepared,
    pub item_logs: BatchOutcome,
    pub ledgers: BatchOutcome,
}
/// Caller already constructed/loaded the source entity; its constructor journal is a separate effect.
/// An interrupted/partially delivered attempt must be reconciled, not blindly retried.
pub async fn deliver(
    pool: &PgPool,
    key: &UserKey<'_>,
    cid: i64,
    profile: &Profile,
    prepare: impl FnOnce(Journals) -> Result<Prepared, DeliveryError>,
) -> Result<Outcome, DeliveryError> {
    fiona_character_state::verify_source_binding(pool, profile.source_state()).await?;
    let tx = fiona_character_state::owned_first_fiona(pool, key, cid).await?;
    tx.commit().await.map_err(InventoryWriteError::Database)?;
    let policy = profile.policy();
    let enabled = |k| {
        policy
            .is_enabled(Some(k))
            .map_err(|_| DeliveryError::Profile)
    };
    let auto = enabled("AutoItemLogging")?;
    let disabled = enabled("DisableLogging")?;
    let disable_operation = enabled("DisableOperationLogging")?;
    let shared = enabled("SharedInventory")?;
    if shared {
        super::shared_item_state::require_empty(pool, key)
            .await
            .map_err(|_| DeliveryError::SharedState)?;
    }
    if enabled("ProcessAfterDeleteItem")? {
        return Err(DeliveryError::Profile);
    }
    let operation = item_journals::get_item_journal_id(pool, disable_operation).await;
    let ledger = item_journals::add_customization_journal(pool, disabled).await;
    let mut prepared = match prepare(Journals {
        operation,
        ledger,
        auto_logging: auto,
        shared_inventory: shared,
    }) {
        Ok(p) => p,
        Err(e) => {
            failure(pool, cid, ledger, disabled, "giveDefaultItemFail").await;
            return Err(e);
        }
    };
    if prepared.inventory.owner() != cid {
        return Err(DeliveryError::Profile);
    }
    let saved = initial_inventory::save_first_fiona(pool, key, &prepared.inventory).await;
    match saved {
        Ok(0) => return Err(DeliveryError::Reconcile),
        Err(e) => {
            prepared.manager.clear();
            failure(pool, cid, ledger, disabled, "giveDefaultItemFail(save)").await;
            return Err(e.into());
        }
        _ => {}
    }
    // Admitted ProcessAfterDeleteItem=false leaves no deleted-items work in a fresh GiveItem.
    let counts = &prepared.counts;
    let item_logs =
        item_event_logs::write_manager(pool, cid, &mut prepared.manager, auto, |d| Counts {
            inventory: counts.get(d.class()).copied().flatten(),
            shared: if shared && counts.get(d.class()).copied().flatten().is_some() {
                Some(0)
            } else {
                None
            },
        })
        .await;
    let ledgers = item_event_logs::write_ledgers(pool, cid, &prepared.ledgers, disabled)
        .await
        .map_err(|_| DeliveryError::Profile)?;
    Ok(Outcome {
        prepared,
        item_logs,
        ledgers,
    })
}
async fn failure(pool: &PgPool, cid: i64, journal: i64, disabled: bool, message: &str) {
    if let Ok(row) = Ledger::creation_failure(
        journal,
        message.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ) {
        let _attempt = item_event_logs::write_ledger(
            pool,
            cid,
            &row,
            disabled,
            tokio::time::Instant::now() + std::time::Duration::from_secs(5),
        )
        .await;
    }
}
