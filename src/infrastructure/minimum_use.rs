//! UseInventoryItemProcessor for the selected first Fiona: the UseInTown dispatch with
//! its dialogs (UseInTown 0 and IsConstraintSatisfied failures) and the capsule_ap
//! branch (UseUseableItem + CharacterImpl.GiveAP) and the RandomItemContents boxes
//! (`minimum_random_item`). Other usable branches (food, RandomItemV2 opening, pets, ...)
//! are rejected without changes until migrated.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::map_partition::OutgoingPacket,
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message, UseItem},
    },
};
use sqlx::PgPool;

pub enum Outcome {
    /// Frontend messages in emission order: UseInventoryItemOk, the GiveAP result
    /// (APMessage or ItemFail), then the queued inventory Sync (UpdateInventoryInfo).
    Applied(Vec<OutgoingPacket>),
    /// Source failure without a frontend message (unknown item ID).
    Unchanged,
    /// A HomeCafeBonus item: the messages, then CharacterCommonInfo (MakeSummary) from the
    /// frontend, which owns the summary query configuration.
    AppliedWithSummary(Vec<OutgoingPacket>),
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// `capsule_ap` amount: the VALUE attribute (verified absent, so 0) and then
/// int.TryParse(ItemClass.Substring(10)); a failed parse yields 0.
fn capsule_amount(class: &str) -> Option<i32> {
    let suffix = class.strip_prefix("capsule_ap")?;
    Some(suffix.parse::<i32>().unwrap_or(0))
}

/// EntityCommunicator.IsConstraintSatisfied(item) with the selected character (no cafe,
/// guild or equipped enhancement in the migrated state).
async fn constraint_failure(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    constraint: &crate::domain::item_constraint::Constraint,
) -> Result<Option<&'static str>, MinimumError> {
    use crate::domain::item_constraint::{self, Character, Equipped};
    let (level, class): (i32, i32) =
        sqlx::query_as("SELECT level,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let skills: Vec<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT skill_utf16,rank FROM game_skills WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let skills = skills
        .into_iter()
        .map(|(k, r)| {
            if !k.len().is_multiple_of(2) || k.len() > 400 {
                return Err(fail());
            }
            let units: Vec<u16> = k
                .as_chunks::<2>()
                .0
                .iter()
                .map(|a| u16::from_le_bytes(*a))
                .collect();
            Ok((String::from_utf16(&units).map_err(|_| fail())?, r))
        })
        .collect::<Result<std::collections::BTreeMap<_, _>, MinimumError>>()?;
    // Only enhancement/enchant requirements read the equipped list (all level 0 here).
    let gear = if constraint.required_enhance > 0 && constraint.required_enchant > 0 {
        Some(super::minimum_equipment::load(tx, owner, assets, cid).await?)
    } else {
        None
    };
    let levels: Vec<Equipped> = gear
        .iter()
        .flat_map(|g| g.projection.equipment.iter())
        .map(|_| Equipped {
            enhance: 0,
            prefix: 0,
            suffix: 0,
        })
        .collect();
    let character = Character {
        level,
        class,
        cafe_type: 0,
        cafe: false,
        guild_level: None,
        skills: &skills,
        equipped: Some(&levels),
    };
    Ok(item_constraint::check(&character, constraint).map(|f| f.name()))
}

/// Admission (selection and UseSecondPassword) belongs to the frontend. `buyback`: the
/// ItemClass of each BuybackList item; `entity_ids` serve items a box creates.
#[allow(clippy::too_many_arguments)]
pub async fn use_item(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    request: UseItem,
    log: super::minimum_item_logs::Context,
    arithmetic: crate::domain::character_stats::Arithmetic,
    buyback: &[String],
    entity_ids: Vec<i64>,
) -> Result<Outcome, MinimumError> {
    // UseInventoryItemProcessor constructor: GetJournalID (UseInventoryItem).
    let journal = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let mut ids = entity_ids.into_iter();
    let mut opened_box = None;
    let (outcome, used) = tokio::time::timeout(std::time::Duration::from_secs(15), async {
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
        // Validates ownership/profile; MIN_UPTIME is refused by the loader and attributed
        // items below, so IsUsable's uptime and broken checks hold for non-equipment
        // (its Expired check is not evaluated here).
        let before =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None)
                .await?;
        let Some(item) = before
            .slots
            .as_ref()
            .ok_or_else(fail)?
            .iter()
            .flatten()
            .find(|s| s.id == request.item_id)
        else {
            // GetItem == null: FailMessage only.
            return Ok((Outcome::Unchanged, None));
        };
        let class = item.class.clone().unwrap_or_default();
        let home_cafe =
            class.starts_with("homecafe_bonus_") || class.starts_with("account_homecafe_bonus_");
        // Item attributes change what some usables do (VALUE amounts, enchant scrolls);
        // only the VALUE of the HomeCafeBonus items is migrated.
        let attributes = item.attributes.as_deref().unwrap_or_default();
        if !attributes.is_empty()
            && !(home_cafe
                && attributes
                    .iter()
                    .all(|a| a.as_ref().and_then(|a| a.name.as_deref()) == Some("VALUE")))
        {
            return Err(fail());
        }
        let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
        // Branches dispatched before the UseInTown test are not migrated.
        if [
            "coupon_bingo_shuffle",
            "pet_",
            "skill_untrain_",
            "guild_advanced_feather",
        ]
        .iter()
        .any(|p| class.starts_with(p))
        {
            return Err(fail());
        }
        let economy = assets.economy.as_ref().ok_or_else(fail)?;
        let info = economy.get(indexed.source_index).ok_or_else(fail)?;
        // Run: ItemLog.AddItemJournal("UseItem") once the item and BadStatus checks pass.
        let use_journal = super::minimum_item_logs::use_item_journal(pool, log.flags).await;
        if !info.use_in_town {
            return Ok((
                Outcome::Applied(vec![super::minimum_equip::item_fail(
                    "UseItemFail_NotUseableItem",
                )?]),
                None,
            ));
        }
        if let Some(failure) =
            constraint_failure(&mut tx, owner, assets, cid, &info.constraint).await?
        {
            return Ok((
                Outcome::Applied(vec![super::minimum_equip::item_fail(failure)?]),
                None,
            ));
        }
        if home_cafe {
            // item.Value.IntValue: int.TryParse of the VALUE attribute, else 0.
            let days = attributes
                .iter()
                .flatten()
                .find(|a| a.name.as_deref() == Some("VALUE"))
                .and_then(|a| a.value.as_deref())
                .and_then(|v| v.parse::<i32>().ok())
                .unwrap_or(0);
            if let Some(dialog) =
                super::minimum_home_cafe::use_check(&mut tx, owner, cid, &class, days).await?
            {
                return Ok((
                    Outcome::Applied(vec![super::minimum_equip::item_fail(dialog)?]),
                    None,
                ));
            }
            // UseUseableItem(1): DestroyItem and Save, then HomeCafeBonus.OnUseItem.
            let deleted = item.count == 1;
            if deleted {
                sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1")
                    .bind(item.id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                    .bind(item.id)
                    .bind(cid)
                    .execute(&mut *tx)
                    .await?;
            } else {
                sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
                    .bind(item.id)
                    .bind(cid)
                    .execute(&mut *tx)
                    .await?;
            }
            let level: i32 =
                sqlx::query_scalar("SELECT level FROM game_character_info WHERE id=$1")
                    .bind(cid)
                    .fetch_one(&mut *tx)
                    .await?;
            let mut packets = vec![OutgoingPacket {
                category: Kind::UseInventoryItemOk.category(),
                body: wire::encode(
                    Kind::UseInventoryItemOk,
                    Some(&Message::UseInventoryItemOk(Some(class.clone()))),
                    DEFAULT_MAX_BODY,
                )
                .map_err(|_| fail())?
                .to_vec(),
            }];
            let changed = super::minimum_home_cafe::use_bonus(
                &mut tx, assets, owner, cid, level, &class, days,
            )
            .await?;
            packets.extend(changed.packets);
            // StatusEffectStat changed: CalculateFinalStat then PushUpdateStat.
            packets.push(
                super::minimum_selection::update_stat(&mut tx, owner, assets, cid, arithmetic)
                    .await?
                    .0,
            );
            let slot = if deleted {
                super::minimum_move::deleted_slot(item.id)
            } else {
                let after = super::minimum_inventory::snapshot_with_equipment(
                    &mut tx, owner, assets, cid, None,
                )
                .await?;
                after
                    .slots
                    .as_ref()
                    .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
                    .cloned()
                    .ok_or_else(fail)?
            };
            let update = Message::UpdateInventoryInfo(Some(vec![Some(slot)]));
            packets.push(OutgoingPacket {
                category: Kind::UpdateInventoryInfo.category(),
                body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
                    .map_err(|_| fail())?
                    .to_vec(),
            });
            tx.commit().await?;
            let change = super::minimum_item_logs::NumChange {
                item_id: item.id,
                class_utf16: key.clone(),
                delta: -1,
            };
            return Ok((
                Outcome::AppliedWithSummary(packets),
                Some((change, use_journal)),
            ));
        }
        let Some(amount) = capsule_amount(&class).filter(|_| !indexed.equippable) else {
            // questscroll_/questscroll2_ and the other branches before the boxes.
            if ["questscroll_", "questscroll2_", "cashshop_trade_coin"]
                .iter()
                .any(|p| class.starts_with(p))
            {
                return Err(fail());
            }
            let Some(opened) = super::minimum_random_item::open(
                &mut tx,
                owner,
                assets,
                cid,
                item,
                request.target_item_id,
                buyback,
                &mut || ids.next().ok_or_else(fail),
            )
            .await?
            else {
                return Err(fail());
            };
            tx.commit().await?;
            let packets = opened.packets.clone();
            opened_box = Some(opened);
            return Ok((Outcome::Applied(packets), None));
        };
        // UseUseableItem: DestroyItem(Useable(item), 1) and Save before onSuccess.
        let deleted = item.count == 1;
        if deleted {
            sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                .bind(item.id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        } else {
            sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
                .bind(item.id)
                .bind(cid)
                .execute(&mut *tx)
                .await?;
        }
        let mut packets = vec![OutgoingPacket {
            category: Kind::UseInventoryItemOk.category(),
            body: wire::encode(
                Kind::UseInventoryItemOk,
                Some(&Message::UseInventoryItemOk(Some(class.clone()))),
                DEFAULT_MAX_BODY,
            )
            .map_err(|_| fail())?
            .to_vec(),
        }];
        if amount == 0 {
            // The item is already consumed when the amount resolves to zero.
            packets.push(super::minimum_equip::item_fail(
                "UseItemFail_NotUseableItem",
            )?);
        } else {
            let ap =
                super::minimum_ap::give_exceeding(&mut tx, owner, assets, cid, arithmetic, amount)
                    .await?;
            packets.push(super::minimum_ap::packet(ap)?);
        }
        let slot = if deleted {
            super::minimum_move::deleted_slot(item.id)
        } else {
            let after = super::minimum_inventory::snapshot_with_equipment(
                &mut tx, owner, assets, cid, None,
            )
            .await?;
            after
                .slots
                .as_ref()
                .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
                .cloned()
                .ok_or_else(fail)?
        };
        let update = Message::UpdateInventoryInfo(Some(vec![Some(slot)]));
        packets.push(OutgoingPacket {
            category: Kind::UpdateInventoryInfo.category(),
            body: wire::encode(Kind::UpdateInventoryInfo, Some(&update), DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
                .to_vec(),
        });
        tx.commit().await?;
        let change = super::minimum_item_logs::NumChange {
            item_id: item.id,
            class_utf16: key.clone(),
            delta: -1,
        };
        Ok((Outcome::Applied(packets), Some((change, use_journal))))
    })
    .await
    .map_err(|_| fail())??;
    if let Some(opened) = opened_box {
        let box_journal =
            super::minimum_item_logs::named_journal(pool, log.flags, "RandomItem").await;
        if let Some(applied) = &opened.applied {
            super::minimum_item_logs::write_give(
                pool,
                cid,
                journal,
                super::minimum_item_logs::USE_INVENTORY_ITEM,
                applied,
                log,
            )
            .await;
        }
        // ItemLog.AddItemLedger(DestroyItem/GiveItem, ActionType.Use).
        for (kind, class, count) in &opened.ledgers {
            super::minimum_item_logs::write_requested_ledger(
                pool,
                cid,
                box_journal,
                *kind,
                9,
                class,
                *count,
                log.flags,
            )
            .await;
        }
    }
    if let Some((change, use_journal)) = used {
        // Save → LogManager.Write (Num −1), then UseUseableItem's ledger.
        super::minimum_item_logs::write_changes(
            pool,
            cid,
            journal,
            super::minimum_item_logs::USE_INVENTORY_ITEM,
            std::slice::from_ref(&change),
            log,
        )
        .await;
        super::minimum_item_logs::write_use_ledger(pool, cid, use_journal, &change, 1, log.flags)
            .await;
    }
    Ok(outcome)
}
