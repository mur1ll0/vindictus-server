//! CraftItemProcessor for the selected first Fiona (RequestCraftMessage → CraftItem): the
//! craft shop recipe, materials and targets times Count, CraftItem(materials, targets with
//! the bind check), Save and the ledgers. TodayMission "TryCraftItem" is not migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        give_item::GiveError, item_attribute, map_partition::OutgoingPacket, shop_price,
        shop_price::Arch,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message},
    },
};
use sqlx::PgPool;

/// ActionType.Craft / EventType.CraftItem / ItemLogType ledgers.
const ACTION_CRAFT: i16 = 11;
const EVENT_CRAFT_ITEM: i16 = 18;
const LOG_GIVE: i16 = 2;
const LOG_DESTROY: i16 = 3;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// Dictionary.AddOrIncrease in first-seen order (unchecked int sums).
fn add_or_increase(rows: &mut Vec<(String, i32)>, class: &str, num: i32) {
    match rows.iter_mut().find(|(c, _)| c == class) {
        Some(row) => row.1 = row.1.wrapping_add(num),
        None => rows.push((class.to_owned(), num)),
    }
}

/// Entity IDs the craft may need: one per crafted target unit (non-stacking targets take a
/// row each), at least one, bounded.
pub fn entity_ids_needed(assets: &Assets, request: &wire::BuyItem) -> usize {
    let recipe = assets.enhance.as_ref().and_then(|c| {
        c.craft_shop(request.shop_id.as_deref()?, request.order)
            .and_then(|id| c.recipe(id))
    });
    let units: i64 = recipe.map_or(0, |r| {
        r.targets
            .iter()
            .map(|(_, n)| i64::from(*n) * i64::from(request.count.max(0)))
            .sum()
    });
    usize::try_from(units.clamp(1, 256)).unwrap_or(1)
}

/// Answers SendOperationFailedDialog; Err keeps the source's FailMessage-only paths.
pub async fn craft(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: &wire::BuyItem,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let dialog = |name: &str| -> Result<Vec<OutgoingPacket>, MinimumError> {
        Ok(vec![super::minimum_equip::item_fail(name)?])
    };
    if request.count <= 0 {
        return Err(fail());
    }
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let policy = contents.policy().ok_or_else(fail)?;
    // ShopID.ToLower() throws on a null ShopID.
    let shop = request.shop_id.as_deref().ok_or_else(fail)?;
    let Some(recipe_id) = contents.craft_shop(shop, request.order) else {
        return dialog("CraftItemFail_NoRecipe");
    };
    let Some(recipe) = contents.recipe(recipe_id) else {
        return dialog("CraftItemFail_NoRecipe");
    };
    let ratio = contents.craft_price_ratio().ok_or_else(fail)?;
    let arch = match assets.item_arithmetic.ok_or_else(fail)? {
        crate::domain::character_stats::Arithmetic::FrameworkX86 => Arch::X86,
        crate::domain::character_stats::Arithmetic::FrameworkX64 => Arch::X64,
    };
    // Recipe.AddMaterial(PriceCalc value), then AddOrIncrease(Num × Count).
    let mut priced = Vec::new();
    for (class, num) in &recipe.materials {
        let value = if class == "gold" {
            shop_price::ceiling10(*num, ratio, arch)
        } else {
            shop_price::ceiling(*num, ratio, arch)
        }
        .ok_or_else(fail)?;
        add_or_increase(&mut priced, class, value);
    }
    let mut materials = Vec::new();
    for (class, num) in &priced {
        add_or_increase(&mut materials, class, num.wrapping_mul(request.count));
    }
    let mut targets = Vec::new();
    for (class, num) in &recipe.targets {
        add_or_increase(&mut targets, class, num.wrapping_mul(request.count));
    }
    // GiveItemRequest.FromDictionaryWithBindCheck: ItemClassEx, a known class and
    // IsCharacterBinded = FeatureMatrix.IsEnable(ItemClassInfo.Bind); IsValidGiveItemRequests
    // refuses an empty or unknown target.
    if targets.is_empty() {
        return dialog("CraftItemFail_System");
    }
    let mut gives = Vec::new();
    for (class_ex, count) in &targets {
        let Ok((class, attributes)) = item_attribute::parse_class_ex(class_ex) else {
            return dialog("CraftItemFail_System");
        };
        let key = utf16(&class);
        let Some(indexed) = assets.stats_index.class(Some(&key)) else {
            return dialog("CraftItemFail_System");
        };
        let bind = assets
            .catalog
            .classes
            .get(indexed.source_index)
            .ok_or_else(fail)?
            .bind
            .clone();
        gives.push((key, *count, policy.is_enabled(bind.as_deref())?, attributes));
    }
    let destroys: Vec<(Vec<u8>, i32)> = materials.iter().map(|(c, n)| (utf16(c), *n)).collect();
    // CraftItemProcessor constructor GetJournalID; AddItemJournal("CraftItem").
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal = super::minimum_item_logs::named_journal(pool, log.flags, "CraftItem").await;
    let mut ids = entity_ids.into_iter();
    let applied = tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let connected: bool =
            sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
                .bind(cid)
                .fetch_one(&mut *tx)
                .await?;
        if !connected {
            return Err(fail());
        }
        sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *tx)
            .await?;
        super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
            .await?;
        let destroy_requests: Vec<_> = destroys
            .iter()
            .map(|(class, count)| super::minimum_give::Request {
                class,
                count: *count,
                bound: false,
                equipment: None,
                attributes: &item_attribute::EMPTY,
            })
            .collect();
        let give_requests: Vec<_> = gives
            .iter()
            .map(
                |(class, count, bound, attributes)| super::minimum_give::Request {
                    class,
                    count: *count,
                    bound: *bound,
                    equipment: None,
                    attributes,
                },
            )
            .collect();
        let applied = super::minimum_give::apply_craft(
            &mut tx,
            owner,
            assets,
            cid,
            &destroy_requests,
            &give_requests,
            &mut || ids.next().ok_or_else(fail),
        )
        .await?;
        if applied.is_ok() {
            tx.commit().await?;
        }
        Ok(applied)
    })
    .await
    .map_err(|_| fail())??;
    let applied = match applied {
        Ok(a) => a,
        // ToCraftItemResult.
        Err(GiveError::NotEnoughItem | GiveError::InvalidRequest) => {
            return dialog("CraftItemFail_NotEnoughMaterial");
        }
        Err(
            e @ (GiveError::ExceedBundleCount
            | GiveError::UniqueConstraint
            | GiveError::NoEmptySlot),
        ) => {
            return dialog(&e.to_string());
        }
        Err(GiveError::InvalidItemClass | GiveError::Unsupported) => return Err(fail()),
    };
    super::minimum_item_logs::write_give(pool, cid, operation, EVENT_CRAFT_ITEM, &applied, log)
        .await;
    for (rows, kind) in [(&materials, LOG_DESTROY), (&targets, LOG_GIVE)] {
        for (class, count) in rows {
            super::minimum_item_logs::write_requested_ledger(
                pool,
                cid,
                journal,
                kind,
                ACTION_CRAFT,
                &utf16(class),
                *count,
                log.flags,
            )
            .await;
        }
    }
    // Save → Sync (UpdateInventoryInfo); the processor answers OkMessage only.
    Ok(vec![OutgoingPacket {
        category: Kind::UpdateInventoryInfo.category(),
        body: wire::encode(
            Kind::UpdateInventoryInfo,
            Some(&Message::UpdateInventoryInfo(Some(applied.slots))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    }])
}
