//! DailyItemController for the selected first Fiona: the procedures of
//! DailyItemProcessInfo run at login, after a HomeCafeBonus use and at each range start
//! during the session (DailyItemScheduler), once per range, recorded in
//! `game_daily_item_process`. The character has no PC cafe (CafeLevel 0); its CafeType is
//! the HomeCafeBonus one. Give/take processes are migrated; fatigue, guild and rank-title
//! processes are skipped without a record, and an item that does not fit the inventory is
//! left for the next range (the source mails it).
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::domain::{
    daily_item::{Procedure, ProcessKind},
    map_partition::OutgoingPacket,
};
use crate::protocol::{
    DEFAULT_MAX_BODY,
    inventory::{self as wire, Kind, Message},
};
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn procedures(assets: &Assets) -> &[Procedure] {
    assets
        .cash_shop
        .as_ref()
        .map_or(&[], |c| c.customize().daily_items.as_slice())
}

async fn held(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    class: &str,
) -> Result<i32, MinimumError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(count),0)::BIGINT FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
    )
    .bind(cid)
    .bind(utf16(class))
    .fetch_one(&mut **tx)
    .await?;
    i32::try_from(count).map_err(|_| fail())
}

/// DailyItemController.Process for the procedures due now: the messages of the given
/// items, then one UpdateInventoryInfo of the changed slots.
pub async fn process(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let list = procedures(assets);
    if list.is_empty() {
        return Ok(Vec::new());
    }
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let now = super::minimum_status_effect::now_ticks()?;
    let row: (i32, i32) =
        sqlx::query_as("SELECT level,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    let (level, base_class) = row;
    let cafe_type = super::minimum_home_cafe::cafe_type(&mut tx, owner.uid, cid).await?;
    let processed: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT process_id,processed_ticks FROM game_daily_item_process WHERE cid=$1",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let mut messages = Vec::new();
    let mut slots = Vec::new();
    for p in list {
        // target.CafeLevel (no PC cafe) and the CafeType requirement.
        if p.cafe_level > 0 || (p.cafe_type != 0 && p.cafe_type != cafe_type) {
            continue;
        }
        let last = processed
            .iter()
            .find(|(id, _)| *id == p.id)
            .map(|(_, t)| *t);
        if !p.is_due(last, now) || !p.fits_level(level) {
            continue;
        }
        let Some(class_ex) = p.args.first() else {
            continue;
        };
        let Some(count) = p.count() else {
            continue;
        };
        let (class, attributes) = match crate::domain::item_attribute::parse_class_ex(class_ex) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let key = utf16(&class);
        let have = held(&mut tx, cid, &class).await?;
        // (destroy, give) counts of the process.
        let (destroy, give) = match &p.kind {
            ProcessKind::GiveItem => (0, count),
            ProcessKind::TakeItem => (count.min(have), 0),
            ProcessKind::GiveItemUntil | ProcessKind::GiveItemUntilByCharacterClass => {
                (0, (count - have).max(0))
            }
            ProcessKind::TakeItemUntil => ((have - count).max(0), 0),
            ProcessKind::NotMigrated(_) => continue,
        };
        if p.kind == ProcessKind::GiveItemUntilByCharacterClass && give > 0 {
            // ItemClassInfo.IsAvailableFor(BaseCharacter).
            let restriction = assets
                .stats_index
                .class(Some(&key))
                .and_then(|c| assets.economy.as_ref()?.get(c.source_index))
                .map_or(0, |e| e.class_restriction);
            if 1_i32
                .checked_shl(u32::try_from(base_class).map_err(|_| fail())?)
                .unwrap_or(0)
                & restriction
                == 0
            {
                continue;
            }
        }
        if destroy > 0 || give > 0 {
            let request = [super::minimum_give::Request {
                class: &key,
                count: destroy.max(give),
                bound: false,
                equipment: None,
                attributes: &attributes,
            }];
            let (destroys, gives): (&[super::minimum_give::Request<'_>], &[_]) = if destroy > 0 {
                (&request, &[])
            } else {
                (&[], &request)
            };
            match super::minimum_give::apply(&mut tx, owner, assets, cid, destroys, gives, ids)
                .await?
            {
                Ok(applied) => slots.extend(applied.slots),
                // The source mails what does not fit; this range is retried later.
                Err(_) => continue,
            }
            if give > 0 && p.kind != ProcessKind::GiveItem {
                messages.push(super::minimum_shop::system_message(
                    super::minimum_shop::SYSTEM,
                    p.message.as_deref().unwrap_or(""),
                    &[class.clone(), give.to_string(), count.to_string()],
                )?);
            }
        }
        // UpdateDailyItemProcess(ID, now).
        sqlx::query("INSERT INTO game_daily_item_process(cid,process_id,processed_ticks) VALUES($1,$2,$3) ON CONFLICT (cid,process_id) DO UPDATE SET processed_ticks=EXCLUDED.processed_ticks")
            .bind(cid)
            .bind(p.id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    if !slots.is_empty() {
        let update = Message::UpdateInventoryInfo(Some(slots));
        messages.push(OutgoingPacket {
            category: Kind::UpdateInventoryInfo.category(),
            body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
                .to_vec(),
        });
    }
    Ok(messages)
}

/// Time until the next range of a periodic procedure starts (DailyItemScheduler).
pub fn next_range(assets: &Assets) -> Result<Option<std::time::Duration>, MinimumError> {
    let now = super::minimum_status_effect::now_ticks()?;
    Ok(procedures(assets)
        .iter()
        .filter(|p| p.cafe_level == 0)
        .filter_map(|p| p.next_range_start(now))
        .min()
        .map(|t| std::time::Duration::from_micros(u64::try_from((t - now) / 10).unwrap_or(0))))
}
