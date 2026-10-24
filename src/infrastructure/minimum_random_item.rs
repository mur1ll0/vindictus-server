//! UseInventoryItemProcessor's RandomItemContents branch (the RandomItem boxes) for the
//! selected first Fiona: the key, level, recipe and ServerCode checks, RandomItemManager
//! Open/OpenBundle, CraftItem(box, rewards) and the result messages. RandomItemV2 boxes,
//! key boxes, Titicore, the fever system, rare/bundle recipe classes and leftovers sent by
//! mail are not migrated and send nothing.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::{
    domain::{item_attribute, map_partition::OutgoingPacket, random_item},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, ColoredEquipment, Kind, Message, RandomItemRewardList, Slot},
    },
};
use sqlx::{Postgres, Transaction};

/// SystemMessageCategory.System / Telepathy.
const SYSTEM: u8 = 0;
const TELEPATHY: u8 = 2;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

struct BoxRandom<'a>(&'a mut crate::domain::item_colors::ColorRandom);
impl random_item::Random for BoxRandom<'_> {
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        self.0.next(maximum).ok()
    }
}

/// The opened box: its packets and the ledgers (DestroyItem/GiveItem, ActionType.Use).
pub(super) struct Opened {
    pub packets: Vec<OutgoingPacket>,
    pub applied: Option<super::minimum_give::Applied>,
    pub ledgers: Vec<(i16, Vec<u8>, i32)>,
}

/// Ok(None): not a RandomItemContents box (the caller keeps its dispatch).
#[allow(clippy::too_many_arguments)]
pub(super) async fn open(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item: &Slot,
    target_item_id: i64,
    buyback: &[String],
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Option<Opened>, MinimumError> {
    let class = item.class.clone().unwrap_or_default();
    let Some(contents) = assets.random_item.as_ref() else {
        return Ok(None);
    };
    let policy = contents.policy();
    // The RandomItemV2 branch comes first.
    if policy.is_enabled(Some("RandomItemV2"))? && contents.is_v2(&class) {
        return Err(fail());
    }
    if !contents.is_random_item(&class) {
        return Ok(None);
    }
    let dialog = |name: &str| -> Result<Option<Opened>, MinimumError> {
        Ok(Some(Opened {
            packets: vec![super::minimum_equip::item_fail(name)?],
            applied: None,
            ledgers: Vec::new(),
        }))
    };
    let keys = contents.keys(&class)?;
    // A key item (TargetItemID) or the SelectTargetItemMessage key choice are not migrated.
    if target_item_id > 0 || (target_item_id < 0 && !keys.is_empty()) {
        return Err(fail());
    }
    let (level, character): (i32, i32) =
        sqlx::query_as("SELECT level,class::integer FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    let key = utf16(&class);
    let required = contents.level(&key).ok_or_else(fail)?;
    if required > level {
        return dialog("ConstraintFail_LowLevel");
    }
    let Some(found) = contents.random_item(&class, None)? else {
        return dialog(if keys.is_empty() {
            "UseItemFail_Titicore_WrongItem"
        } else {
            "UseItemFail_Titicore_NeedKeyItem"
        });
    };
    let found = found.clone();
    if found
        .server_code
        .is_some_and(|c| policy.get_integer("GameCode").is_ok_and(|g| g != c))
    {
        return dialog("UseItemFail_NotUseableItem");
    }
    if (policy.is_enabled(Some("Titicore"))? && class.starts_with("randomitem_titicore"))
        || policy.is_enabled(Some("RandomItem_FeverSystem"))?
    {
        return Err(fail());
    }
    let bundle = policy.is_enabled(Some("RandomItemBundle"))?;
    // ItemOwner.GetItems() (every owned item) and BuybackList, for variable recipes.
    let held: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT DISTINCT item_class_utf16 FROM game_items WHERE owner_id=$1 LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let holds = |c: &str| held.contains(&utf16(c)) || buyback.iter().any(|b| b == c);
    let storage = |c: &str| {
        assets
            .stats_index
            .class(Some(&utf16(c)))
            .and_then(|i| assets.catalog.classes.get(i.source_index))
            .map(|r| r.storage)
    };
    let owner_view = random_item::Owner {
        holds: &holds,
        storage: &storage,
        constraint_check: policy.is_enabled(Some("VariableItem_ConstraintCheck"))?,
        variable_probability: policy.is_enabled(Some("VariableProbability_RandomItem"))?,
    };
    let classes = contents.recipe_classes(found.id);
    let recipes = |name: &str| contents.recipes(found.id, name);
    let (rewards, notes) = {
        let mut random = assets.random_item_random.lock().await;
        random_item::open(
            classes,
            &recipes,
            character,
            Some(&owner_view),
            bundle,
            &mut BoxRandom(&mut random),
        )
        .map_err(|_| fail())?
    };
    let mut packets = Vec::new();
    if notes.variable_dialog {
        packets.push(super::minimum_equip::item_fail("UseItemFail_RandomItem")?);
    }
    // CraftItem: DestroyItem(ItemID(box), 1) with the reward requests.
    let deleted = item.count <= 1;
    if deleted {
        super::minimum_enhance::delete_item(tx, cid, item.id).await?;
    } else {
        sqlx::query("UPDATE game_items SET count=count-1 WHERE id=$1 AND owner_id=$2")
            .bind(item.id)
            .bind(cid)
            .execute(&mut **tx)
            .await?;
    }
    let mut parsed = Vec::new();
    for r in &rewards {
        let (name, attributes) =
            item_attribute::parse_class_ex(&r.item_class).map_err(|_| fail())?;
        let class_key = utf16(&name);
        let equippable = assets
            .stats_index
            .class(Some(&class_key))
            .ok_or_else(fail)?
            .equippable;
        let equipment = match r.colors {
            [-1, -1, -1] => None,
            colors if equippable && !colors.contains(&-1) => Some(super::minimum_give::Equipment {
                colors,
                reduced: 0,
                bonus: 0,
            }),
            // Partly colored or colored non-equipment requests are not migrated.
            _ => return Err(fail()),
        };
        let shown = item_attribute::build_class_ex(&name, &attributes).map_err(|_| fail())?;
        parsed.push((class_key, r.number, equipment, attributes, shown));
    }
    let requests: Vec<Request<'_>> = parsed
        .iter()
        .map(|(class, count, equipment, attributes, _)| Request {
            class,
            count: *count,
            bound: false,
            equipment: *equipment,
            attributes,
        })
        .collect();
    let applied = super::minimum_give::apply_craft(tx, owner, assets, cid, &[], &requests, ids)
        .await?
        // GiveItemFail_NoEmptySlot sends the leftovers by mail, which is not migrated.
        .map_err(|_| fail())?;
    let mut slots: Vec<Option<Slot>> = vec![Some(if deleted {
        super::minimum_move::deleted_slot(item.id)
    } else {
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .ok_or_else(fail)?
            .into_iter()
            .flatten()
            .find(|s| s.id == item.id)
            .ok_or_else(fail)?
    })];
    slots.extend(applied.slots.iter().cloned());
    packets.push(packet(
        Kind::UpdateInventoryInfo,
        Message::UpdateInventoryInfo(Some(slots)),
    )?);
    packets.push(packet(
        Kind::UseInventoryItemOk,
        Message::UseInventoryItemOk(Some(class.clone())),
    )?);
    packets.push(super::minimum_shop::system_message(
        SYSTEM,
        "GameUI_Heroes_RandomItem_UseItem",
        std::slice::from_ref(&class),
    )?);
    let mut colored = Vec::new();
    let mut numbers = Vec::new();
    let mut ledgers = vec![(3, key.clone(), 1)];
    for (r, (class_key, count, _, _, shown)) in rewards.iter().zip(&parsed) {
        packets.push(match &r.notify {
            Some(text) => super::minimum_shop::system_message(TELEPATHY, text, &[])?,
            None => super::minimum_shop::system_message(
                SYSTEM,
                "GameUI_Heroes_RandomItem_GetItem",
                &[shown.clone(), count.to_string()],
            )?,
        });
        ledgers.push((2, class_key.clone(), *count));
        colored.push(Some(ColoredEquipment {
            item_class: Some(shown.clone()),
            color1: r.colors[0],
            color2: r.colors[1],
            color3: r.colors[2],
        }));
        numbers.push(*count);
    }
    packets.push(packet(
        Kind::RandomItemRewardList,
        Message::RandomItemRewardList(if found.visual == Some(1) {
            RandomItemRewardList {
                item_classes: Some(colored),
                item_quantities: Some(numbers),
                is_user_care: false,
                key_item_class: Some(String::new()),
                disable_show_pop_up: false,
            }
        } else {
            // RandomItemRewardListMessage(disableShowPopUp: true, keyItemClass).
            RandomItemRewardList {
                item_classes: None,
                item_quantities: None,
                is_user_care: false,
                key_item_class: Some(String::new()),
                disable_show_pop_up: true,
            }
        }),
    )?);
    Ok(Some(Opened {
        packets,
        applied: Some(applied),
        ledgers,
    }))
}
