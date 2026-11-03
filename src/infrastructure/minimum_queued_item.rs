//! QueuedItemController.ProcessQueuedItem for the entered first Fiona: the unprocessed
//! QueuedItem rows of the character (ProcessDailyItemProcessor at login and each daily
//! range, AllUserGoalRewardProcessor), each in its own ItemTransactionScope. Rows of an
//! existing system item (ItemID) are not migrated and stay queued; a row with a mail
//! title that is not one plain stack stays queued too. The item ledger is not written.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::domain::{give_item::GiveError, mail::mail_type, map_partition::OutgoingPacket};
use sqlx::PgPool;

/// Loader.RemoveQueuedItem: Processed with the processing time.
async fn processed(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    row_id: i32,
    now_units: i64,
) -> Result<(), MinimumError> {
    sqlx::query(
        "UPDATE game_queued_items SET processed=TRUE,processed_time_units300=$2 WHERE row_id=$1",
    )
    .bind(row_id)
    .bind(now_units)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(bytes: &[u8]) -> String {
    String::from_utf16_lossy(
        &bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect::<Vec<_>>(),
    )
}

/// The packets of the processed rows and how many event rows (EventCode) were met.
#[derive(Default)]
pub struct Processed {
    pub packets: Vec<OutgoingPacket>,
    pub event_rows: i32,
}

type Row = (
    i32,
    Vec<u8>,
    i32,
    bool,
    Option<Vec<u8>>,
    Option<Vec<u8>>,
    Option<i64>,
    Option<Vec<u8>>,
    Option<i64>,
);

/// SQL datetime units (1/300 s since 1900-01-01) as DateTime ticks.
fn units_ticks(units: i64) -> i64 {
    599_266_080_000_000_000 + units * 100_000 / 3
}

/// ProcessQueuedItem(itemFestival): ItemFestival is not migrated, so every row is processed.
pub async fn process(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Processed, MinimumError> {
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT row_id,item_class_ex_utf16,count,is_character_binded,mail_title_utf16,mail_content_utf16,item_id,event_code_utf16,expire_time_units300 FROM game_queued_items WHERE cid=$1 AND NOT processed ORDER BY row_id LIMIT 256",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    let now_units = super::minimum_quest::now_units()?;
    let now_ticks = super::minimum_status_effect::now_ticks()?;
    let mut out = Processed::default();
    let mut slots = Vec::new();
    for (row_id, class_ex, count, bound, title, content, item_id, event, expire) in rows {
        if event.is_some() {
            out.event_rows += 1;
        }
        let mut sp = sqlx::Acquire::begin(&mut *tx).await?;
        let expire_ticks = expire.map(units_ticks);
        // An expired row is dropped (QueuedItemExpired).
        if expire_ticks.is_some_and(|e| e <= now_ticks) {
            processed(&mut sp, row_id, now_units).await?;
            sp.commit().await?;
            continue;
        }
        if item_id.is_some() {
            tracing::warn!("queued system item not migrated; row left queued; values omitted");
            sp.rollback().await?;
            continue;
        }
        let class_ex = text(&class_ex);
        let Ok((class, attributes)) = crate::domain::item_attribute::parse_class_ex(&class_ex)
        else {
            sp.rollback().await?;
            continue;
        };
        let key = utf16(&class);
        // GetItemClassInfo null: the row stays.
        let Some(bind) = assets.stats_index.bind(&key) else {
            sp.rollback().await?;
            continue;
        };
        // DefaultCharacterBinderChecker: never for gold and silver coins.
        let bound = (bound || assets.catalog.policy.is_enabled(bind)?)
            && class != "gold"
            && class != "silver_coin";
        let event = event.as_deref().map(text);
        let kind = match event.as_deref() {
            Some("GuildLevelUp") => 12,
            Some("MobileAppBuy") => mail_type::AUCTION_ITEM,
            Some("FriendEventReward_Add") => mail_type::DUNGEON_ITEM,
            _ => 11,
        };
        let success = super::minimum_shop::system_message(
            super::minimum_shop::SYSTEM,
            "GameUI_Heroes_GiveQueuedItem_Success",
            &[class_ex.clone(), count.to_string()],
        )?;
        if let (Some(title), Some(content)) = (title, content) {
            let (title, content) = (text(&title), text(&content));
            match super::minimum_mail::mail_new_system_item(
                &mut sp,
                assets,
                cid,
                (&key, count, &attributes, bound),
                kind,
                (&title, &content),
                expire_ticks,
                ids,
            )
            .await?
            {
                Some(_) => {
                    processed(&mut sp, row_id, now_units).await?;
                    sp.commit().await?;
                    out.packets.push(success);
                }
                None => {
                    tracing::warn!("queued mail item is not one plain stack; row left queued");
                    sp.rollback().await?;
                }
            }
            continue;
        }
        let request = [Request {
            class: &key,
            count,
            bound,
            equipment: None,
            attributes: &attributes,
        }];
        match super::minimum_give::apply_leftover(
            &mut sp,
            owner,
            assets,
            cid,
            &[],
            &request,
            true,
            ids,
        )
        .await?
        {
            Ok(applied) => {
                processed(&mut sp, row_id, now_units).await?;
                // The leftovers go by mail with the event title.
                for item in &applied.system {
                    let class = text(&item.class_utf16);
                    let title =
                        format!("#GameUI_Heroes_Event_GiveItem_Title,{class},{}", item.count);
                    let content = format!(
                        "#GameUI_Heroes_Event_GiveItem_Content,{class},{}",
                        item.count
                    );
                    super::minimum_mail::send_system_item_mail(
                        &mut sp,
                        assets,
                        cid,
                        item.id,
                        kind,
                        Some((&title, &content)),
                        expire_ticks,
                    )
                    .await?;
                    super::minimum_mail::notice(cid);
                }
                sp.commit().await?;
                slots.extend(applied.slots);
                out.packets.push(success);
            }
            // UnknownError: retried at the next login.
            Err(GiveError::Unsupported) => sp.rollback().await?,
            // Another result removes the row without an item, as the source.
            Err(_) => {
                processed(&mut sp, row_id, now_units).await?;
                sp.commit().await?;
            }
        }
    }
    tx.commit().await?;
    if !slots.is_empty() {
        out.packets
            .insert(0, super::minimum_mission::inventory_packet(slots)?);
    }
    Ok(out)
}
