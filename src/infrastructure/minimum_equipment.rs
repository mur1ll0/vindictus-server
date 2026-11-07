//! Readonly, source-bound normal Gear loader for the initialized first Fiona.
//! No repair of invalid parts, equipment mutations, journal allocation or Save.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::domain::{
    character_stats::Stats,
    equipped_gear::{self, Projection, SetBonus},
    item_durability::EquipmentDurability,
};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::BTreeMap;
pub struct Loaded {
    pub projection: Projection,
    pub bonus: SetBonus,
}
#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    slot: i32,
    expire_ticks: Option<i64>,
    part: i16,
    color1: i32,
    color2: i32,
    color3: i32,
    reduced_durability: i32,
    max_durability_bonus: i32,
}
fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    if raw.len() > 1024 || !raw.len().is_multiple_of(2) {
        return Err(fail());
    }
    String::from_utf16(
        &raw.as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .map_err(|_| fail())
}
/// StorageSetting.GetStorageTypeOf: Cash (100) and Story (103) collections.
const CASH_COLLECTION: i16 = 100;
const STORY_COLLECTION: i16 = 103;
/// Item.LookClassInfo class: the LOOK attribute value, else the item class.
pub(super) fn look_class(
    class: &str,
    attributes: &crate::domain::item_attribute::Attributes,
) -> String {
    attributes
        .get("LOOK")
        .map_or_else(|| class.to_owned(), |a| a.value.clone())
}
/// BadStatus StatusFail_NotFreePassCouponSetted: a free pass coupon whose look is still a
/// free pass coupon (no LOOK chosen in the beauty shop).
pub(super) fn free_pass_unset(class: &str, look: &str) -> bool {
    class.starts_with("cash_freepass_") && look.starts_with("cash_freepass_")
}
/// Item.IsExpired: ExpireDateTime < DateTime.UtcNow (ticks). An expired equipped item
/// keeps its slot with BadStatus StatusFail_Expired (EquippableSlot.ItemDisabled).
pub(super) fn expired(expire_ticks: Option<i64>) -> Result<bool, MinimumError> {
    let Some(expire) = expire_ticks else {
        return Ok(false);
    };
    Ok(expire < super::lab_clock::utc_now().map_err(|_| fail())?.ticks())
}
/// Password/city admission belongs to the frontend; ownership/provenance is
/// independently checked here. The caller retains all locks until packet construction.
pub async fn query(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Loaded, MinimumError> {
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let loaded = load(&mut tx, owner, assets, cid).await?;
        tx.commit().await?;
        Ok(loaded)
    })
    .await
    .map_err(|_| fail())?
}
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Loaded, MinimumError> {
    let marker:Option<(i32,Vec<u8>,bool)>=sqlx::query_as("SELECT uid,profile_sha256,initialized FROM game_minimum_character_initialization WHERE cid=$1 FOR SHARE").bind(cid).fetch_optional(&mut **tx).await?;
    // The marker records the initializer of the character's class.
    if !marker.is_some_and(|(uid, fingerprint, done)| {
        done && uid == owner.uid && assets.initializers().by_fingerprint(&fingerprint).is_some()
    }) {
        return Err(fail());
    }
    sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN SHARE MODE").execute(&mut **tx).await?;
    let rows:Vec<Row>=sqlx::query_as("SELECT i.id,i.item_class_utf16,i.count,i.collection,i.slot,i.expire_ticks,e.part,e.color1,e.color2,e.color3,e.reduced_durability,e.max_durability_bonus FROM game_items i JOIN game_equippables e ON e.id=i.id WHERE i.owner_id=$1 AND e.part IS NOT NULL ORDER BY i.id LIMIT 4097").bind(cid).fetch_all(&mut **tx).await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    if rows.is_empty() {
        let gear = assets.appearance.initial_gear(&assets.catalog)?;
        let infos = gear
            .rows()
            .iter()
            .map(|i| equipped_gear::Info {
                part: i.part,
                costume: i.costume_part,
                decoration: i.decoration,
                serial: -1,
                colors: [0; 3],
                level: 0,
                weight: 0.0,
                durability: 0,
                maximum: 0,
                armor_hp: 0,
                stats: Stats::default(),
                destroyed_def: 0,
                effect: None,
                class: String::new(),
                equip_class: String::new(),
                is_avatar: false,
                category: String::new(),
            })
            .collect();
        return Ok(Loaded {
            projection: Projection {
                infos,
                equipment: vec![],
            },
            bonus: SetBonus {
                stats: Stats::default(),
                skills: BTreeMap::new(),
            },
        });
    }
    let profile = assets.equipment.as_ref().ok_or_else(fail)?;
    profile.bind(&assets.catalog)?;
    let stats = &assets.stats_index;
    let ids: Vec<_> = rows.iter().map(|r| r.id).collect();
    let mut attributes = super::minimum_inventory::load_attributes(tx, &ids).await?;
    let owner_context = super::minimum_item_stat::context(tx, cid).await?;
    let names: Vec<_> = Stats::default()
        .entries()
        .map(|(s, _)| s.encode_utf16().collect::<Vec<_>>())
        .collect();
    let mut items = Vec::new();
    let mut metadata = Vec::new();
    let bags = super::minimum_inventory::bags(tx, cid).await?;
    let story_tab = assets
        .catalog
        .policy
        .is_enabled(Some("StoryTabInInventory"))?;
    // Gear.GetAvatarPart answers -1 without the Avatar feature: no avatar look then.
    let avatar_on = assets.catalog.policy.is_enabled(Some("Avatar"))?;
    let parts = assets.appearance.normal_parts(&assets.catalog)?;
    for row in rows {
        // StorageSetting.EquippableItemStorage: Bag, Cash (100) and Story (103 with
        // StoryTabInInventory); a Cash or Story location only needs Slot >= 0.
        let located = (bags.contains(&row.collection) && (0..48).contains(&row.slot))
            || ((row.collection == CASH_COLLECTION
                || (story_tab && row.collection == STORY_COLLECTION))
                && row.slot >= 0);
        if row.count != 1 || !located {
            return Err(fail());
        }
        let class = text(&row.item_class_utf16)?;
        let m = profile.metadata(&assets.catalog, &row.item_class_utf16)?;
        let base = stats.get(Some(&row.item_class_utf16)).ok_or_else(fail)?;
        let row_attributes = attributes.remove(&row.id).unwrap_or_default();
        // Item.LookClassInfo: the LOOK attribute's class, else the item's own; its
        // EquipItemInfo.CostumeType is the CostumeSN of the part.
        let look = look_class(&class, &row_attributes);
        let serial = if look == class {
            m.serial
        } else {
            profile
                .metadata(
                    &assets.catalog,
                    &look
                        .encode_utf16()
                        .flat_map(u16::to_le_bytes)
                        .collect::<Vec<_>>(),
                )?
                .serial
        };
        // EquippableSlot.ToEquippedItemInfo: Data.Stat, MaxDurability and Enhance.Effect.
        let item = super::minimum_item_stat::equipment_with(
            assets,
            &row.item_class_utf16,
            &row_attributes,
            Some(&owner_context),
        )?;
        let d = EquipmentDurability::load(
            base,
            &item.stat,
            row.reduced_durability,
            row.max_durability_bonus,
        );
        let fields = item.stat.fields();
        let values = Stats::from_values(std::array::from_fn(|n| item.stat.get(Some(&names[n]))));
        let disabled = (d.has_durability() && d.maximum() >= 0 && d.current() == 0)
            || expired(row.expire_ticks)?
            || free_pass_unset(&class, &look);
        items.push(equipped_gear::Item {
            id: row.id,
            part: u8::try_from(row.part).map_err(|_| fail())?,
            category: m.category.clone(),
            class,
            equip_class: m.equip_class.clone(),
            serial,
            level: m.level,
            colors: [row.color1, row.color2, row.color3],
            weight: fields[1],
            durability: d.current(),
            maximum: d.maximum(),
            armor_hp: fields[3],
            stats: values,
            destroyed_def: fields[2],
            effect: item.effect,
            disabled,
            looks: if avatar_on
                && row.part > i16::from(equipped_gear::AVATAR_OFFSET)
                && row.part < 100
            {
                avatar_looks(&m.extra_data, profile, assets, &parts)
            } else {
                Vec::new()
            },
        });
        metadata.push(m);
    }
    let bonus = profile.bonus(&items, &metadata)?;
    let projection = Projection::load(&parts, &items).map_err(|_| fail())?;
    Ok(Loaded { projection, bonus })
}

/// Gear.ToEquippedItemInfos' reading of an avatar piece's ExtraData: CostumeKey/DecorationKey names
/// hide that costume part; an item class with EquipItemInfo shows its CostumeType on the first
/// part of its category that has a costume part (ItemContents.GetEquippablePart order).
fn avatar_looks(
    extra: &str,
    profile: &super::equipment_profile::Profile,
    assets: &Assets,
    parts: &[equipped_gear::Part],
) -> Vec<equipped_gear::AvatarLook> {
    use equipped_gear::AvatarLook;
    let mut out = Vec::new();
    for token in extra
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .take(64)
    {
        if let Some(costume) = equipped_gear::costume_key(token) {
            out.push(AvatarLook::Hide {
                costume,
                decoration: false,
            });
            continue;
        }
        if let Some(costume) = equipped_gear::decoration_key(token) {
            out.push(AvatarLook::Hide {
                costume,
                decoration: true,
            });
            continue;
        }
        let key: Vec<u8> = token.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let Ok(look) = profile.metadata(&assets.catalog, &key) else {
            continue;
        };
        if let Some(p) = parts
            .iter()
            .find(|p| p.category == look.category && p.costume.is_some())
        {
            out.push(AvatarLook::Show {
                costume: p.costume.unwrap_or(-1),
                decoration: p.decoration,
                serial: look.serial,
            });
        }
    }
    out
}
