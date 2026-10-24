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
    if marker != Some((owner.uid, assets.initializer.fingerprint().to_vec(), true)) {
        return Err(fail());
    }
    super::item_loader_state::require_expanded_absent(tx, cid)
        .await
        .map_err(|_| fail())?;
    sqlx::query("LOCK TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status IN SHARE MODE").execute(&mut **tx).await?;
    let storage: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_item_storage_status WHERE owner_id=$1)",
    )
    .bind(cid)
    .fetch_one(&mut **tx)
    .await?;
    if storage {
        return Err(fail());
    }
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
    for row in rows {
        if row.count != 1
            || row.expire_ticks.is_some()
            || !matches!(row.collection, 0 | 1 | 6)
            || !(0..48).contains(&row.slot)
        {
            return Err(fail());
        }
        let class = text(&row.item_class_utf16)?;
        if class.starts_with("cash_freepass_") {
            return Err(fail());
        }
        let m = profile.metadata(&assets.catalog, &row.item_class_utf16)?;
        let base = stats.get(Some(&row.item_class_utf16)).ok_or_else(fail)?;
        // EquippableSlot.ToEquippedItemInfo: Data.Stat, MaxDurability and Enhance.Effect.
        let item = super::minimum_item_stat::equipment_with(
            assets,
            &row.item_class_utf16,
            &attributes.remove(&row.id).unwrap_or_default(),
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
        let disabled = d.has_durability() && d.maximum() >= 0 && d.current() == 0;
        items.push(equipped_gear::Item {
            id: row.id,
            part: u8::try_from(row.part).map_err(|_| fail())?,
            category: m.category.clone(),
            class,
            equip_class: m.equip_class.clone(),
            serial: m.serial,
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
        });
        metadata.push(m);
    }
    let bonus = profile.bonus(&items, &metadata)?;
    let projection = Projection::load(&assets.appearance.normal_parts(&assets.catalog)?, &items)
        .map_err(|_| fail())?;
    Ok(Loaded { projection, bonus })
}
