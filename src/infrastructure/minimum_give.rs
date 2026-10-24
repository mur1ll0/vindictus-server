//! ItemOwner.ProcessItemRequest (all-or-nothing overload) for the selected first Fiona:
//! plans with domain::give_item, applies VirtualModifyItem/VirtualCreateItem in the
//! caller's transaction and returns the Sync slot diff. Gold keeps its count checksum.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_item_logs::NumChange,
};
use crate::{
    domain::{
        give_item::{self, ClassInfo, Destroy, Give, GiveError, Held, Inventory, Plan},
        initial_inventory::{self, gold_checksum},
        item_attribute::{self, Attributes, StackKeys},
        item_location::ItemLocation,
    },
    protocol::inventory::Slot,
};
use sqlx::{Postgres, Transaction};

/// One ItemClass (UTF-16LE) and count; IsCharacterBinded false as GiveItem(string, int).
pub struct Request<'a> {
    pub class: &'a [u8],
    pub count: i32,
    /// GiveItemRequest IsCharacterBinded (ignored for destroy requests).
    pub bound: bool,
    /// ItemRequestProperty colors and durability of an existing equipment (buyback);
    /// None draws material colors with full durability.
    pub equipment: Option<Equipment>,
    /// ItemRequestProperty.Attribute (ItemClassEx attributes or a sold item's clone).
    pub attributes: &'a Attributes,
}
/// Equippable fields kept by Item.ToRequestInfo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Equipment {
    pub colors: [i32; 3],
    pub reduced: i32,
    pub bonus: i32,
}
/// Item created by MakeItem, for LogManager CreateItem records.
pub struct CreatedItem {
    pub id: i64,
    pub class_utf16: Vec<u8>,
    pub count: i32,
    pub colors: [i32; 3],
    pub reduced: i32,
    pub bonus: i32,
}
pub struct Applied {
    /// UpdateInventoryInfo diff: modified items (deleted as empty SlotInfo), then created.
    pub slots: Vec<Option<Slot>>,
    pub changes: Vec<NumChange>,
    pub created: Vec<CreatedItem>,
}
#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    item_class_utf16: Vec<u8>,
    count: i32,
    collection: i16,
    slot: i32,
    tradable: bool,
    expire_ticks: Option<i64>,
}

fn units(class: &[u8]) -> Vec<u16> {
    class
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect()
}

/// Effective ItemClassInfo of a request; unknown classes are InvalidItemClass.
pub(super) fn class_info(assets: &Assets, class: &[u8]) -> Result<Option<ClassInfo>, MinimumError> {
    let Some(indexed) = assets.stats_index.class(Some(class)) else {
        return Ok(None);
    };
    let economy = assets
        .economy
        .as_ref()
        .and_then(|e| e.get(indexed.source_index))
        .ok_or(MinimumError::Conflict)?;
    let tircoin = assets
        .catalog
        .policy
        .is_enabled(Some("ingame_cashshop_tircoin_system"))?
        && assets.inventory.contains_tircoin(class);
    Ok(Some(ClassInfo {
        class: class.to_vec(),
        max_stack: indexed.max_stack,
        storage: indexed.storage,
        unique: economy.unique,
        equippable: indexed.equippable,
        expire_in: economy.expire_in,
        tircoin,
    }))
}

/// Caller holds the owned-character transaction; this locks the item tables for
/// writing and validates the inventory profile before planning.
pub(super) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    destroys: &[Request<'_>],
    gives: &[Request<'_>],
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Result<Applied, GiveError>, MinimumError> {
    if destroys.len() + gives.len() > 256 {
        return Err(MinimumError::Conflict);
    }
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None).await?;
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id,item_class_utf16,count,collection,slot,tradable,expire_ticks FROM game_items WHERE owner_id=$1 ORDER BY id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let held_ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
    let attributes = super::minimum_inventory::load_attributes(tx, &held_ids).await?;
    let mut keys = StackKeys::default();
    // ItemRequestProperty.Build_Attribute_Expire/MinUpTime turn these into
    // ExpireDateTime and a formatted UtcNow, which this profile does not port.
    if gives.iter().any(|g| {
        g.attributes.get(item_attribute::EXPIRE).is_some()
            || g.attributes.get(item_attribute::MIN_UPTIME).is_some()
    }) {
        return Ok(Err(GiveError::Unsupported));
    }
    let held = rows
        .iter()
        .map(|r| {
            Ok((
                r.item_class_utf16.clone(),
                Held {
                    id: r.id,
                    count: r.count,
                    location: ItemLocation {
                        collection: u8::try_from(r.collection)
                            .map_err(|_| MinimumError::Conflict)?,
                        slot: r.slot,
                    },
                    bound: !r.tradable,
                    expires: r.expire_ticks.is_some(),
                    attributes: attributes.get(&r.id).map_or(0, |a| keys.intern(a)),
                },
            ))
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let mut infos = Vec::with_capacity(destroys.len() + gives.len());
    for r in destroys.iter().chain(gives) {
        match class_info(assets, r.class)? {
            Some(info) => infos.push(info),
            None => return Ok(Err(GiveError::InvalidItemClass)),
        }
    }
    let (maximum_durability, bundle_exceed) = assets.inventory.limits()?;
    let story_tab = assets
        .catalog
        .policy
        .is_enabled(Some("StoryTabInInventory"))?;
    let (destroy_infos, give_infos) = infos.split_at(destroys.len());
    let destroy_requests: Vec<Destroy<'_>> = destroys
        .iter()
        .zip(destroy_infos)
        .map(|(r, class)| Destroy {
            class,
            count: r.count,
        })
        .collect();
    let give_requests: Vec<Give<'_>> = gives
        .iter()
        .zip(give_infos)
        .map(|(r, class)| Give {
            class,
            count: r.count,
            bound: r.bound,
            attributes: keys.intern(r.attributes),
        })
        .collect();
    let inventory = Inventory {
        items: &held,
        bags: &[0, 1, 6],
        bundle_exceed,
        story_tab,
    };
    let plan = match give_item::plan(&inventory, &destroy_requests, &give_requests) {
        Ok(plan) => plan,
        Err(e) => return Ok(Err(e)),
    };
    let properties: Vec<(Option<Equipment>, &Attributes)> =
        gives.iter().map(|g| (g.equipment, g.attributes)).collect();
    let applied = write(
        tx,
        owner,
        assets,
        cid,
        &held,
        &infos,
        plan,
        &properties,
        maximum_durability,
        ids,
    )
    .await?;
    Ok(Ok(applied))
}

#[allow(clippy::too_many_arguments)]
async fn write(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    held: &[(Vec<u8>, Held)],
    infos: &[ClassInfo],
    plan: Plan,
    properties: &[(Option<Equipment>, &Attributes)],
    maximum_durability: i32,
    ids: &mut (dyn FnMut() -> Result<i64, MinimumError> + Send),
) -> Result<Applied, MinimumError> {
    let mut changes = Vec::new();
    let mut deleted = Vec::new();
    for &(id, count) in &plan.modified {
        let (class, item) = held
            .iter()
            .find(|(_, h)| h.id == id)
            .ok_or(MinimumError::Conflict)?;
        let info = infos
            .iter()
            .find(|i| i.class == *class)
            .ok_or(MinimumError::Conflict)?;
        if info.equippable {
            // An equipped item would also leave the Gear (unequip, stats and equipment
            // sync), which this slice does not migrate; a bag item is deleted with its
            // Equippable row. MaxStack 1 makes every destroy reach 0.
            let part: Option<Option<i16>> =
                sqlx::query_scalar("SELECT part FROM game_equippables WHERE id=$1 FOR UPDATE")
                    .bind(id)
                    .fetch_optional(&mut **tx)
                    .await?;
            if count != 0 || part.flatten().is_some() {
                return Err(MinimumError::Conflict);
            }
            sqlx::query("DELETE FROM game_equippables WHERE id=$1")
                .bind(id)
                .execute(&mut **tx)
                .await?;
        }
        if count == 0 {
            delete_attributes(tx, id).await?;
            sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
                .bind(id)
                .bind(cid)
                .execute(&mut **tx)
                .await?;
            deleted.push(id);
        } else {
            let checksum = assets
                .inventory
                .is_gold(class)
                .then(|| gold_checksum(id, Some(cid), &units(class), count));
            sqlx::query("UPDATE game_items SET count=$3,checksum=$4 WHERE id=$1 AND owner_id=$2")
                .bind(id)
                .bind(cid)
                .bind(count)
                .bind(checksum)
                .execute(&mut **tx)
                .await?;
        }
        changes.push(NumChange {
            item_id: id,
            class_utf16: class.clone(),
            delta: count - item.count,
        });
    }
    let mut created = Vec::with_capacity(plan.created.len());
    {
        let mut random = assets.random.lock().await;
        for c in &plan.created {
            let info = infos
                .iter()
                .find(|i| i.class == c.class)
                .ok_or(MinimumError::Conflict)?;
            let id = ids()?;
            let gold = assets.inventory.is_gold(&c.class);
            let checksum = gold.then(|| gold_checksum(id, Some(cid), &units(&c.class), c.count));
            // Item.Initialize: ExpireIn sets ExpireDateTime = UtcNow + ExpireIn (sampled per
            // item); SubmitChanges stores it as SQL datetime, read back millisecond-rounded.
            let expire = crate::domain::item_time::initialize_expiry(
                None,
                false,
                info.expire_in,
                &mut || {
                    super::lab_clock::utc_now()
                        .map_err(|_| crate::domain::item_time::ItemTimeError::Clock)
                },
            )
            .map_err(|_| MinimumError::Conflict)?
            .expiry
            .map(|t| {
                t.sql_parameter()
                    .and_then(crate::domain::legacy_user_data::SqlDateTime::framework_date_time)
                    .map(|t| t.ticks())
            })
            .transpose()
            .map_err(|_| MinimumError::Conflict)?;
            sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum) VALUES($1,$2,$3,$4,$5,$6,$9,$7,$8)")
                .bind(id)
                .bind(&c.class)
                .bind(cid)
                .bind(c.count)
                .bind(i16::from(c.location.collection))
                .bind(c.location.slot)
                .bind(!c.bound)
                .bind(checksum)
                .bind(expire)
                .execute(&mut **tx)
                .await?;
            let (given, attributes) = properties
                .get(c.request)
                .copied()
                .ok_or(MinimumError::Conflict)?;
            insert_attributes(tx, id, attributes).await?;
            let (colors, reduced, bonus) = if let (true, Some(e)) = (info.equippable, given) {
                sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,NULL,$2,$3,$4,$5,$6)")
                    .bind(id)
                    .bind(e.colors[0])
                    .bind(e.colors[1])
                    .bind(e.colors[2])
                    .bind(e.reduced)
                    .bind(e.bonus)
                    .execute(&mut **tx)
                    .await?;
                (e.colors, e.reduced, e.bonus)
            } else if info.equippable {
                let base = assets
                    .stats_index
                    .get(Some(&c.class))
                    .ok_or(MinimumError::Conflict)?;
                let names = assets
                    .color_index
                    .get(&c.class)
                    .ok_or(MinimumError::Conflict)?;
                let e = initial_inventory::new_equipment(
                    base,
                    names,
                    assets.colors_materials(),
                    &mut random,
                    maximum_durability,
                )
                .map_err(|_| MinimumError::Conflict)?;
                sqlx::query("INSERT INTO game_equippables(id,part,color1,color2,color3,reduced_durability,max_durability_bonus) VALUES($1,NULL,$2,$3,$4,$5,$6)")
                    .bind(id)
                    .bind(e.colors[0])
                    .bind(e.colors[1])
                    .bind(e.colors[2])
                    .bind(e.reduced)
                    .bind(e.bonus)
                    .execute(&mut **tx)
                    .await?;
                (e.colors, e.reduced, e.bonus)
            } else {
                ([-1; 3], 0, 0)
            };
            created.push(CreatedItem {
                id,
                class_utf16: c.class.clone(),
                count: c.count,
                colors,
                reduced,
                bonus,
            });
        }
    }
    let owner_rows =
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .ok_or(MinimumError::Conflict)?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
    let mut slots = Vec::with_capacity(changes.len() + created.len());
    for change in &changes {
        slots.push(Some(if deleted.contains(&change.item_id) {
            super::minimum_move::deleted_slot(change.item_id)
        } else {
            owner_rows
                .iter()
                .find(|s| s.id == change.item_id)
                .cloned()
                .ok_or(MinimumError::Conflict)?
        }));
    }
    for c in &created {
        slots.push(Some(
            owner_rows
                .iter()
                .find(|s| s.id == c.id)
                .cloned()
                .ok_or(MinimumError::Conflict)?,
        ));
    }
    Ok(Applied {
        slots,
        changes,
        created,
    })
}

/// Item.InitAttribute rows, inserted in Attribute.Values order so their identity
/// order is the load order.
pub(super) async fn insert_attributes(
    tx: &mut Transaction<'_, Postgres>,
    item: i64,
    attributes: &Attributes,
) -> Result<(), MinimumError> {
    if attributes.is_empty() {
        return Ok(());
    }
    let ready: bool =
        sqlx::query_scalar("SELECT initialized FROM game_item_identity WHERE singleton FOR SHARE")
            .fetch_one(&mut **tx)
            .await?;
    if !ready {
        return Err(MinimumError::Conflict);
    }
    for a in attributes.as_slice() {
        sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(nextval('game_item_attribute_id'),$1,$2,$3,$4,$5)")
            .bind(item)
            .bind(a.name.as_bytes())
            .bind(a.value.as_bytes())
            .bind(a.arg)
            .bind(a.arg2)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
/// ItemAttribute rows go with their item (no FK in the source schema).
pub(super) async fn delete_attributes(
    tx: &mut Transaction<'_, Postgres>,
    item: i64,
) -> Result<(), MinimumError> {
    sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1")
        .bind(item)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Laboratory grant (no source counterpart): GiveItem(itemClass, num) for an
/// offline first Fiona, so the client receives the change through InventoryInfo
/// on the next selection. IDs come from a separate entity Service.ID.
pub async fn lab_grant(
    pool: &sqlx::PgPool,
    assets: &Assets,
    cid: Option<i64>,
    class: &str,
    count: i32,
    service_id: i32,
) -> Result<Result<usize, GiveError>, MinimumError> {
    let cid = match cid {
        Some(cid) => cid,
        None => {
            let all: Vec<i64> = sqlx::query_scalar(
                "SELECT cid FROM game_minimum_character_initialization WHERE initialized LIMIT 2",
            )
            .fetch_all(pool)
            .await?;
            match all[..] {
                [cid] => cid,
                _ => return Err(MinimumError::Conflict),
            }
        }
    };
    if class.is_empty()
        || class.len() > 512
        || !class.is_ascii()
        || !(1..=i32::MAX).contains(&count)
    {
        return Err(MinimumError::Conflict);
    }
    // An ItemClassEx ("class[NAME:value(arg)<arg2>,...]"), as GiveItem(string, int) parses.
    let (class, attributes) =
        item_attribute::parse_class_ex(class).map_err(|_| MinimumError::Conflict)?;
    if class.is_empty() || class.len() > 50 {
        return Err(MinimumError::Conflict);
    }
    let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let user: Option<(i32, String, i64)> = sqlx::query_as(
        "SELECT u.id,u.name,u.local_revision FROM game_character_info c JOIN game_users u ON u.id=c.uid WHERE c.id=$1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await?;
    let (uid, name, revision) = user.ok_or(MinimumError::Conflict)?;
    let owner = UserKey {
        uid,
        name: &name,
        revision,
    };
    let flags = super::minimum_item_logs::flags(assets)?;
    let journal = super::minimum_item_logs::operation_journal(pool, flags).await;
    let mut authority = crate::application::character_creation::EntityAuthority::new(service_id);
    let mut ids = move || {
        let now = super::lab_clock::utc_now().map_err(|_| MinimumError::Conflict)?;
        authority
            .allocate(now)
            .map(|a| a.entity_time.ticks())
            .map_err(|_| MinimumError::Conflict)
    };
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, &owner, cid).await?;
    let connected: bool =
        sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if connected {
        return Err(MinimumError::Conflict);
    }
    let applied = match apply(
        &mut tx,
        &owner,
        assets,
        cid,
        &[],
        &[Request {
            class: &key,
            count,
            bound: false,
            equipment: None,
            attributes: &attributes,
        }],
        &mut ids,
    )
    .await?
    {
        Ok(applied) => applied,
        Err(e) => return Ok(Err(e)),
    };
    tx.commit().await?;
    super::minimum_item_logs::write_give(
        pool,
        cid,
        journal,
        super::minimum_item_logs::LAB_GRANT,
        &applied,
        super::minimum_item_logs::Context {
            flags,
            civil_offset_seconds: 0,
        },
    )
    .await;
    Ok(Ok(applied.slots.len()))
}

impl Applied {
    /// The log records of this change, without the slot diff.
    pub fn clone_for_logs(&self) -> Self {
        Self {
            slots: Vec::new(),
            changes: self
                .changes
                .iter()
                .map(|c| NumChange {
                    item_id: c.item_id,
                    class_utf16: c.class_utf16.clone(),
                    delta: c.delta,
                })
                .collect(),
            created: self
                .created
                .iter()
                .map(|c| CreatedItem {
                    id: c.id,
                    class_utf16: c.class_utf16.clone(),
                    count: c.count,
                    colors: c.colors,
                    reduced: c.reduced,
                    bonus: c.bonus,
                })
                .collect(),
        }
    }
}
