//! InGame QueryCharacterViewInfoMessage (the "Character Info" entry of a player's menu):
//! QueryCIDByName → WhoIs(Stat|Title) on the target's PlayerService and QueryTargetInventory
//! → CharacterViewInfoMessage(QueryID, Summary, Stats, QuickSlotInfo, EquipmentDictionary,
//! silver_coin, platinum_coin + platinum_coin_pc, DuriabilityDictionary). With the Arena
//! feature (on in the EU, `docs/social-systems.md`) the summary carries the target's p05
//! PvpResultInfo. An unknown name or a failed aggregation sends nothing, as the source.
use super::{
    character_lobby::{QueryConfig, UserKey},
    minimum_assets::Assets,
    minimum_character::MinimumError,
};
use crate::{
    domain::{character_stats::Arithmetic, map_partition::OutgoingPacket},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self, ColoredEquipment, Dict, DurabilityEquipment, ViewInfoTail},
        minimum_map::{self, Kind, Message},
    },
};
use sqlx::PgPool;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// The CharacterViewInfoMessage for the character named `name`, or None when no living
/// character has that name.
pub async fn query(
    pool: &PgPool,
    assets: &Assets,
    query_id: i64,
    name: &str,
    config: QueryConfig,
    arithmetic: Arithmetic,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        // QueryCIDByName.
        let Some((cid, uid)) = super::minimum_party::character_info_by_name(pool, name).await?
        else {
            return Ok(None);
        };
        let user: Option<(String, i64)> =
            sqlx::query_as("SELECT name,local_revision FROM game_users WHERE id=$1")
                .bind(uid)
                .fetch_optional(pool)
                .await?;
        let Some((user_name, revision)) = user else {
            return Ok(None);
        };
        let owner = UserKey {
            uid,
            name: &user_name,
            revision,
        };
        let mut tx = pool.begin().await?;
        // WhoIs(Stat|Title): the summary and Stats.GetDictionary.
        let view = super::minimum_selection::view(
            &mut tx, &owner, assets, cid, config, arithmetic, false, query_id,
        )
        .await?;
        // QueryTargetInventory: the target's InventoryInfo.
        let inventory =
            super::minimum_inventory::snapshot_with_equipment(&mut tx, &owner, assets, cid, None)
                .await?;
        let arena: Option<(i32, i32, Option<i32>)> = sqlx::query_as(
            "SELECT win,lose,max_successive_win_count FROM game_pvp_result_info WHERE cid=$1 AND pvp_mode='p05'",
        )
        .bind(cid)
        .fetch_optional(&mut *tx)
        .await?;
        tx.commit().await?;
        let tail = tail(inventory)?;
        let Some(Message::CharacterCommonInfo { info, .. }) =
            minimum_map::decode(Kind::CharacterCommonInfo, &view.common.body, DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
        else {
            return Err(fail());
        };
        let mut info = info.ok_or_else(fail)?;
        if let Some((win, lose, successive)) = arena {
            info.arena_win_count = win;
            info.arena_lose_count = lose;
            // PvpResultInfoGet결과.MaxSuccessiveWinCount.Value.
            info.arena_successive_win_count = successive.ok_or_else(fail)?;
        }
        let Some(Message::UpdateStat { stat }) =
            minimum_map::decode(Kind::UpdateStat, &view.stat.body, DEFAULT_MAX_BODY)
                .map_err(|_| fail())?
        else {
            return Err(fail());
        };
        let message = Message::CharacterViewInfo {
            query_id,
            info: Some(info),
            stat,
            tail: &tail,
        };
        let body = minimum_map::encode(Kind::CharacterViewInfo, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?;
        Ok(Some(OutgoingPacket {
            category: Kind::CharacterViewInfo.category(),
            body: body.to_vec(),
        }))
    })
    .await
    .map_err(|_| fail())?
}

/// QueryTargetInventory's QuickSlotInfo, EquipmentDictionary (ItemClassExBuilder.Build and
/// colors by equipped part), coin counts and DuriabilityDictionary (MaxDurabilityBonus,
/// MaxDurability − Durability), encoded after the Stat dictionary.
fn tail(inventory: inventory::Inventory) -> Result<Vec<u8>, MinimumError> {
    let slots: Vec<inventory::Slot> = inventory.slots.into_iter().flatten().flatten().collect();
    let count = |class: &str| -> i32 {
        slots
            .iter()
            .filter(|s| s.class.as_deref() == Some(class))
            .fold(0_i32, |n, s| n.wrapping_add(s.count))
    };
    let mut equipment = Vec::new();
    let mut durability = Vec::new();
    for (part, item_id) in inventory.equipment.0.into_iter().flatten() {
        let Some(slot) = slots.iter().find(|s| s.id == item_id) else {
            continue;
        };
        let class = slot.class.clone().unwrap_or_default();
        let attributes = slot
            .attributes
            .iter()
            .flatten()
            .flatten()
            .map(|a| crate::domain::item_attribute::Attribute {
                name: a.name.clone().unwrap_or_default(),
                value: a.value.clone().unwrap_or_default(),
                arg: a.arg,
                arg2: a.arg2,
            })
            .collect();
        let attributes =
            crate::domain::item_attribute::Attributes::new(attributes).map_err(|_| fail())?;
        let class_ex = crate::domain::item_attribute::build_class_ex(&class, &attributes)
            .map_err(|_| fail())?;
        let color = |i: usize| {
            slot.colors
                .as_ref()
                .and_then(|c| c.get(i).copied())
                .unwrap_or(0)
        };
        equipment.push((
            part,
            Some(ColoredEquipment {
                item_class: Some(class_ex),
                color1: color(0),
                color2: color(1),
                color3: color(2),
            }),
        ));
        durability.push((
            part,
            Some(DurabilityEquipment {
                max_durability_bonus: slot.bonus,
                diff_durability: slot.maximum.wrapping_sub(slot.durability),
            }),
        ));
    }
    let tail = ViewInfoTail {
        quick_slots: inventory.quick_slots,
        equipment: Dict(Some(equipment)),
        silver_coin: count("silver_coin"),
        platinum_coin: count("platinum_coin").wrapping_add(count("platinum_coin_pc")),
        durability: Dict(Some(durability)),
    };
    inventory::encode_view_info_tail(&tail, DEFAULT_MAX_BODY).map_err(|_| fail())
}
