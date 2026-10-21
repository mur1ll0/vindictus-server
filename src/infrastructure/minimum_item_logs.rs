//! ItemLog/ItemLogManager effects of the selected-character item processors:
//! the processor's GetJournalID, Num-change logs (AutoItemLogging) written after
//! Save, and explicit ledgers. Logging failures never undo the saved operation.
use super::{
    item_event_logs, item_journals, minimum_assets::Assets, minimum_character::MinimumError,
};
use crate::domain::item_logs::{Counts, Data, Ledger, Manager};
use sqlx::PgPool;

/// EventType values of the recovered processors.
pub const EQUIP_BUNDLE: i16 = 32;
pub const EQUIP_ITEM: i16 = 33;
pub const MOVE_INVENTORY_ITEM: i16 = 53;
pub const USE_INVENTORY_ITEM: i16 = 68;
/// EventType.DestroyItem (DestroyItemProcessor's SetItemOpLog).
pub const DESTROY_ITEM: i16 = 23;
/// EventType.Cheat: the laboratory grant mirrors the GM GiveItem cheat.
pub const LAB_GRANT: i16 = 1;
/// EventTypeDetail.CreateItem (ItemFactory.MakeItem).
const DETAIL_CREATE_ITEM: i16 = 1;
/// ItemLogType.DestroyItem and ActionType.Use for UseUseableItem's ledger.
const LEDGER_DESTROY_ITEM: i16 = 3;
const ACTION_USE: i16 = 9;

/// Logging policy and the civil clock offset (LogInfo uses DateTime.Now).
#[derive(Clone, Copy)]
pub struct Context {
    pub flags: Flags,
    pub civil_offset_seconds: i32,
}
#[derive(Clone, Copy)]
pub struct Flags {
    auto: bool,
    disable_logging: bool,
    disable_operation: bool,
    shared: bool,
}
pub fn flags(assets: &Assets) -> Result<Flags, MinimumError> {
    let enabled = |key: &str| assets.catalog.policy.is_enabled(Some(key));
    Ok(Flags {
        auto: enabled("AutoItemLogging")?,
        disable_logging: enabled("DisableLogging")?,
        disable_operation: enabled("DisableOperationLogging")?,
        shared: enabled("SharedInventory")?,
    })
}
/// Processor constructor: ItemLog.GetJournalID, allocated even when the run fails.
pub async fn operation_journal(pool: &PgPool, flags: Flags) -> i64 {
    item_journals::get_item_journal_id(pool, flags.disable_operation).await
}
/// ItemLog.AddItemJournal(name).
pub async fn named_journal(pool: &PgPool, flags: Flags, name: &str) -> i64 {
    item_journals::add_item_journal(pool, flags.disable_logging, name).await
}
/// ItemImpl.DestroyItem: AddItemJournal("DestroyItem") before the destroy.
pub async fn destroy_item_journal(pool: &PgPool, flags: Flags) -> i64 {
    item_journals::add_destroy_item_journal(pool, flags.disable_logging).await
}
pub async fn use_item_journal(pool: &PgPool, flags: Flags) -> i64 {
    item_journals::add_use_item_journal(pool, flags.disable_logging).await
}

/// Item.Num setter → LogManager.Add(LogData.From(owner, item, delta)).
pub struct NumChange {
    pub item_id: i64,
    pub class_utf16: Vec<u8>,
    pub delta: i32,
}

async fn bundle_count(pool: &PgPool, cid: i64, class: &[u8]) -> Option<i32> {
    // ItemOwner.GetBundleItemCount over the located items of the class.
    sqlx::query_scalar::<_, Option<i64>>(
        "SELECT sum(count) FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
    )
    .bind(cid)
    .bind(class)
    .fetch_one(pool)
    .await
    .ok()
    .map(|sum| sum.and_then(|s| i32::try_from(s).ok()).unwrap_or(0))
}

fn data(change: &NumChange, count: i32) -> Option<Data> {
    // Non-equippable stacks: no colors or durability (LogData of a non-equippable item).
    Data::new(
        change.item_id,
        change.class_utf16.clone(),
        count,
        [-1; 3],
        0,
        0,
        false,
    )
    .ok()
}

/// LogManager.Write after a successful Save: one record per Num change, the last
/// one with live owner counts.
pub async fn write_changes(
    pool: &PgPool,
    cid: i64,
    journal: i64,
    event: i16,
    changes: &[NumChange],
    context: Context,
) {
    let flags = context.flags;
    if !flags.auto || changes.is_empty() {
        return;
    }
    let mut manager = Manager::default();
    manager.set_journal(journal, event);
    let now = || super::lab_clock::civil_now(context.civil_offset_seconds).ok();
    for change in changes {
        let (Some(d), Some(captured)) = (data(change, change.delta), now()) else {
            tracing::warn!("minimum item log record skipped; values omitted");
            continue;
        };
        if manager.add(d, true, || captured).is_err() {
            tracing::warn!("minimum item log record skipped; values omitted");
        }
    }
    let last = changes
        .last()
        .map(|c| c.class_utf16.clone())
        .unwrap_or_default();
    let inventory = bundle_count(pool, cid, &last).await;
    let outcome = item_event_logs::write_manager(pool, cid, &mut manager, true, |_| Counts {
        inventory,
        shared: if flags.shared && inventory.is_some() {
            Some(0)
        } else {
            None
        },
    })
    .await;
    if outcome.failed > 0 {
        tracing::warn!(
            failed = outcome.failed,
            "minimum item logs partially written"
        );
    }
}

/// UseUseableItem: AddItemLedger(journal, DestroyItem, Use, LogData.From(owner, item, n)).
pub async fn write_use_ledger(
    pool: &PgPool,
    cid: i64,
    journal: i64,
    change: &NumChange,
    used: i32,
    flags: Flags,
) {
    let Some(d) = data(change, used) else {
        return;
    };
    let ledger = Ledger {
        journal,
        event: LEDGER_DESTROY_ITEM,
        action: ACTION_USE,
        data: d,
        bundle: bundle_count(pool, cid, &change.class_utf16).await,
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let _attempt =
        item_event_logs::write_ledger(pool, cid, &ledger, flags.disable_logging, deadline).await;
}

/// GiveItem effects after Save: Num-change records of the modified items, then the
/// MakeItem CreateItem records, the last one with live owner counts.
pub async fn write_give(
    pool: &PgPool,
    cid: i64,
    journal: i64,
    event: i16,
    applied: &super::minimum_give::Applied,
    context: Context,
) {
    let flags = context.flags;
    if !flags.auto || (applied.changes.is_empty() && applied.created.is_empty()) {
        return;
    }
    let mut manager = Manager::default();
    manager.set_journal(journal, event);
    let now = || super::lab_clock::civil_now(context.civil_offset_seconds).ok();
    for change in &applied.changes {
        let (Some(d), Some(captured)) = (data(change, change.delta), now()) else {
            tracing::warn!("minimum item log record skipped; values omitted");
            continue;
        };
        if manager.add(d, true, || captured).is_err() {
            tracing::warn!("minimum item log record skipped; values omitted");
        }
    }
    for item in &applied.created {
        let record = Data::new(
            item.id,
            item.class_utf16.clone(),
            item.count,
            item.colors,
            item.reduced,
            item.bonus,
            true,
        );
        let (Ok(d), Some(captured)) = (record, now()) else {
            tracing::warn!("minimum item log record skipped; values omitted");
            continue;
        };
        if manager
            .add_detail(d, DETAIL_CREATE_ITEM, true, || captured)
            .is_err()
        {
            tracing::warn!("minimum item log record skipped; values omitted");
        }
    }
    let last = applied
        .created
        .last()
        .map(|c| c.class_utf16.clone())
        .or_else(|| applied.changes.last().map(|c| c.class_utf16.clone()))
        .unwrap_or_default();
    let inventory = bundle_count(pool, cid, &last).await;
    let outcome = item_event_logs::write_manager(pool, cid, &mut manager, true, |_| Counts {
        inventory,
        shared: if flags.shared && inventory.is_some() {
            Some(0)
        } else {
            None
        },
    })
    .await;
    if outcome.failed > 0 {
        tracing::warn!(
            failed = outcome.failed,
            "minimum item logs partially written"
        );
    }
}

/// ItemImpl.DestroyItem after Save: AddItemLedgers(journal, DestroyItem, Unknown,
/// itemlist.ToLogData) with LogData.From(owner, itemClass, num).
pub async fn write_destroy_ledger(
    pool: &PgPool,
    cid: i64,
    journal: i64,
    class: &[u8],
    count: i32,
    flags: Flags,
) {
    let Ok(d) = Data::requested(class.to_vec(), count) else {
        return;
    };
    let ledger = Ledger {
        journal,
        event: LEDGER_DESTROY_ITEM,
        action: 0,
        data: d,
        bundle: bundle_count(pool, cid, class).await,
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let _attempt =
        item_event_logs::write_ledger(pool, cid, &ledger, flags.disable_logging, deadline).await;
}

/// AddItemLedger(journal, event, action, LogData.From(owner, itemClass, num)).
#[allow(clippy::too_many_arguments)]
pub async fn write_requested_ledger(
    pool: &PgPool,
    cid: i64,
    journal: i64,
    event: i16,
    action: i16,
    class: &[u8],
    count: i32,
    flags: Flags,
) {
    let Ok(d) = Data::requested(class.to_vec(), count) else {
        return;
    };
    let ledger = Ledger {
        journal,
        event,
        action,
        data: d,
        bundle: bundle_count(pool, cid, class).await,
    };
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    let _attempt =
        item_event_logs::write_ledger(pool, cid, &ledger, flags.disable_logging, deadline).await;
}
