//! PetManager of PlayerService (1.69) over game_pets, game_pet_skills, game_pet_feeds and
//! game_pet_ledger: the list, names, creation by pet_create_ items, renaming by
//! pet_name_change items, selection, slots, removal, food (pet_feed_ items and the shared
//! food of Pet_food_V2), cash skills, the end-of-battle experience, hunger and active time,
//! and the periodic active time reset. Pets belong to the account (OwnerUID); every write
//! runs in the owned character's account transaction.
//!
//! Item uses check the pet operation before the item is consumed. The source destroys the
//! item first and gives it back (or mails it) when the operation fails; both leave the
//! inventory as it was and send the same dialog.
//!
//! The battle side (MicroPlayerPetManager) is `BattlePets`, kept in the mission.
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        map_partition::OutgoingPacket,
        pet::{self as rules, Contents, Pet, PetResult, PetSkill, Settings, status},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as inventory_wire, Slot},
        pet::{
            self as wire, DateTimeData, Message, PetFeedElement, PetSkillElement, PetStatElement,
            PetStatusInfo,
        },
    },
};
use sqlx::{PgPool, Postgres, Transaction};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicI64, Ordering},
};

/// SystemMessageCategory.System and Dialog.
const SYSTEM: u8 = 0;
const DIALOG: u8 = 4;
/// Bound of the pets one account may hold (PetManager.MaxPetSlotSize).
const MAX_PETS: i64 = rules::MAX_PET_SLOTS as i64;

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// FeatureMatrix values of the pet system: IsEnable through the NPC policy (its measured
/// version pairs), the numbers from the pet export.
pub fn settings(assets: &Assets) -> Result<Settings, MinimumError> {
    let policy = assets.npc_policy().ok_or_else(fail)?;
    let contents = contents(assets)?;
    let int = |key| contents.feature_int(key).ok_or_else(fail);
    Ok(Settings {
        enabled: policy.is_enabled(Some("Pet"))?,
        food_v2: policy.is_enabled(Some("Pet_food_V2"))?,
        purchase_limit: int("PetPurchaseLimit")?,
        max_level: int("PetMaxLevel")?,
        exp_battle_rate: f64::from(contents.feature_float("Pet_ExpBattle_Point").unwrap_or(0.0))
            / 100.0,
        name_max_bytes: int("NamingRuleMaxBytes")?,
    })
}
fn contents(assets: &Assets) -> Result<&Contents, MinimumError> {
    assets.pets().ok_or_else(fail)
}
fn now() -> Result<i64, MinimumError> {
    super::minimum_status_effect::now_ticks()
}
/// Service.MakeEntityID().Ticks: DateTime ticks, strictly increasing in this process.
fn entity_id() -> Result<i64, MinimumError> {
    static LAST: AtomicI64 = AtomicI64::new(0);
    let ticks = now()?;
    let mut last = LAST.load(Ordering::Relaxed);
    loop {
        let next = ticks.max(last + 1);
        match LAST.compare_exchange(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(next),
            Err(seen) => last = seen,
        }
    }
}

type PetRow = (
    i64,
    i32,
    String,
    i32,
    i32,
    i32,
    i16,
    i32,
    i32,
    i32,
    i32,
    Option<i32>,
    Option<i64>,
    Option<i64>,
);
type SkillRow = (i64, i64, i32, i32, i32, i16, Option<i64>);

/// PetManager.Load: the account's pets in PetID order with their cash skills.
async fn load(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    lock: bool,
) -> Result<Vec<Pet>, MinimumError> {
    let sql = if lock {
        "SELECT id,owner_uid,name,pet_type,stat_type,slot,status,level,exp,total_exp,hunger,remain_active_time,deactive_start_ticks,expired_ticks FROM game_pets WHERE owner_uid=$1 ORDER BY id LIMIT 101 FOR UPDATE"
    } else {
        "SELECT id,owner_uid,name,pet_type,stat_type,slot,status,level,exp,total_exp,hunger,remain_active_time,deactive_start_ticks,expired_ticks FROM game_pets WHERE owner_uid=$1 ORDER BY id LIMIT 101"
    };
    let rows: Vec<PetRow> = sqlx::query_as(sql).bind(uid).fetch_all(&mut **tx).await?;
    if rows.len() as i64 > MAX_PETS {
        return Err(fail());
    }
    let skills: Vec<SkillRow> = sqlx::query_as(
        "SELECT s.row_id,s.pet_id,s.slot,s.skill_id,s.open_level,s.cool_time_type,s.expired_ticks FROM game_pet_skills s JOIN game_pets p ON p.id=s.pet_id WHERE p.owner_uid=$1 ORDER BY s.row_id LIMIT 1001",
    )
    .bind(uid)
    .fetch_all(&mut **tx)
    .await?;
    if skills.len() > 1000 {
        return Err(fail());
    }
    rows.into_iter()
        .map(|r| {
            Ok(Pet {
                id: r.0,
                owner_uid: r.1,
                name: r.2,
                pet_type: r.3,
                stat_type: r.4,
                slot: r.5,
                status: u8::try_from(r.6).map_err(|_| fail())?,
                level: r.7,
                exp: r.8,
                total_exp: r.9,
                hunger: r.10,
                remain_active_time: r.11,
                deactive_start_ticks: r.12,
                expired_ticks: r.13,
                skills: skills
                    .iter()
                    .filter(|s| s.1 == r.0)
                    .map(|s| {
                        Ok(PetSkill {
                            row_id: s.0,
                            slot: s.2,
                            skill_id: s.3,
                            open_level: s.4,
                            cool_time_type: u8::try_from(s.5).map_err(|_| fail())?,
                            expired_ticks: s.6,
                        })
                    })
                    .collect::<Result<_, MinimumError>>()?,
            })
        })
        .collect()
}
async fn insert(tx: &mut Transaction<'_, Postgres>, pet: &Pet) -> Result<(), MinimumError> {
    sqlx::query(
        "INSERT INTO game_pets(id,owner_uid,name,pet_type,stat_type,slot,status,level,exp,total_exp,hunger,remain_active_time,deactive_start_ticks,expired_ticks) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)",
    )
    .bind(pet.id)
    .bind(pet.owner_uid)
    .bind(&pet.name)
    .bind(pet.pet_type)
    .bind(pet.stat_type)
    .bind(pet.slot)
    .bind(i16::from(pet.status))
    .bind(pet.level)
    .bind(pet.exp)
    .bind(pet.total_exp)
    .bind(pet.hunger)
    .bind(pet.remain_active_time)
    .bind(pet.deactive_start_ticks)
    .bind(pet.expired_ticks)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
/// Context.SubmitChanges of a changed Pet row.
async fn save(tx: &mut Transaction<'_, Postgres>, pet: &Pet) -> Result<(), MinimumError> {
    let changed = sqlx::query(
        "UPDATE game_pets SET name=$2,slot=$3,status=$4,level=$5,exp=$6,total_exp=$7,hunger=$8,remain_active_time=$9,deactive_start_ticks=$10,expired_ticks=$11 WHERE id=$1 AND owner_uid=$12",
    )
    .bind(pet.id)
    .bind(&pet.name)
    .bind(pet.slot)
    .bind(i16::from(pet.status))
    .bind(pet.level)
    .bind(pet.exp)
    .bind(pet.total_exp)
    .bind(pet.hunger)
    .bind(pet.remain_active_time)
    .bind(pet.deactive_start_ticks)
    .bind(pet.expired_ticks)
    .bind(pet.owner_uid)
    .execute(&mut **tx)
    .await?
    .rows_affected();
    if changed != 1 {
        return Err(fail());
    }
    Ok(())
}
/// UserSharedItemGet(UID, 2): FeedType → count.
async fn feeds(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
) -> Result<BTreeMap<u8, i32>, MinimumError> {
    let rows: Vec<(i16, i32)> = sqlx::query_as(
        "SELECT feed_type,count FROM game_pet_feeds WHERE owner_uid=$1 ORDER BY feed_type FOR UPDATE",
    )
    .bind(uid)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|(t, c)| Ok((u8::try_from(t).map_err(|_| fail())?, c)))
        .collect()
}
/// UserSharedItemSet(UID, 2, FeedType, count); 0 removes the row.
async fn set_feed(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    feed_type: u8,
    count: i32,
) -> Result<(), MinimumError> {
    if count <= 0 {
        sqlx::query("DELETE FROM game_pet_feeds WHERE owner_uid=$1 AND feed_type=$2")
            .bind(uid)
            .bind(i16::from(feed_type))
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query("INSERT INTO game_pet_feeds(owner_uid,feed_type,count) VALUES($1,$2,$3) ON CONFLICT(owner_uid,feed_type) DO UPDATE SET count=EXCLUDED.count")
            .bind(uid)
            .bind(i16::from(feed_type))
            .bind(count)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
/// The changed rows of the shared food after an in-memory feeding.
async fn store_feeds(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    before: &BTreeMap<u8, i32>,
    after: &BTreeMap<u8, i32>,
) -> Result<(), MinimumError> {
    for (t, c) in before {
        let now = after.get(t).copied().unwrap_or(0);
        if now != *c {
            set_feed(tx, uid, *t, now).await?;
        }
    }
    Ok(())
}
/// PetLog.AddPetLedger (LogOperation name, PetOperationResult name, two arguments).
async fn ledger(
    tx: &mut Transaction<'_, Postgres>,
    pet_id: i64,
    uid: i32,
    operation: &str,
    result: PetResult,
    arg1: &str,
    arg2: &str,
) -> Result<(), MinimumError> {
    let clip = |s: &str| s.chars().take(100).collect::<String>();
    sqlx::query("INSERT INTO game_pet_ledger(pet_id,owner_uid,operation,event,arg1,arg2) VALUES($1,$2,$3,$4,$5,$6)")
        .bind(pet_id)
        .bind(uid)
        .bind(operation)
        .bind(result.name())
        .bind(clip(arg1))
        .bind(clip(arg2))
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// MakePetStatusInfo: default skills then cash skills, the level's balance (zeros without
/// one), no accessories and RemainExpiredTime = UtcNow.
pub(super) fn status_info(pet: &Pet, contents: &Contents, now: i64) -> PetStatusInfo {
    let mut skills: Vec<Option<PetSkillElement>> = contents
        .default_skills(pet.pet_type)
        .iter()
        .map(|d| {
            Some(PetSkillElement {
                skill_id: d.skill_id,
                slot_order: d.slot_order,
                open_level: d.open_level,
                has_expire_date_time_info: false,
                expire_date_time_diff: -1,
            })
        })
        .collect();
    skills.extend(pet.skills.iter().map(|s| {
        Some(PetSkillElement {
            skill_id: s.skill_id,
            slot_order: s.slot,
            open_level: s.open_level,
            has_expire_date_time_info: s.expired_ticks.is_some(),
            expire_date_time_diff: s.expired_ticks.map_or(-1, |e| e - now),
        })
    }));
    let stat = contents
        .balance(pet.stat_type, pet.level)
        .map(|b| PetStatElement {
            required_exp: b.required_exp,
            max_exp: b.max_exp,
            hp: b.hp,
            res_damage: b.res_damage,
            hp_recovery: b.hp_recovery,
            def_break: b.def_break,
            atk_balance: b.atk_balance,
            atk: b.atk,
            def: b.defense,
            critical: b.critical,
            res_critical: b.res_critical,
        })
        .unwrap_or_default();
    PetStatusInfo {
        pet_id: pet.id,
        pet_name: Some(pet.name.clone()),
        pet_type: pet.pet_type,
        slot: pet.slot,
        level: pet.level,
        exp: pet.exp,
        desire: pet.hunger,
        stat: Some(stat),
        skills: Some(skills),
        accessories: Some(Vec::new()),
        pet_status: pet.status,
        remain_active_time: pet.remain_active_time.unwrap_or(0),
        remain_expired_time: DateTimeData::utc(now),
    }
}
fn packet(message: Message) -> Result<OutgoingPacket, MinimumError> {
    let kind = message.kind();
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}
fn list_packet(
    pets: &[&Pet],
    contents: &Contents,
    total: bool,
    now: i64,
) -> Result<OutgoingPacket, MinimumError> {
    packet(Message::PetList(wire::PetList {
        pets: Some(
            pets.iter()
                .map(|p| Some(status_info(p, contents, now)))
                .collect(),
        ),
        is_total: total,
    }))
}
fn feed_packet(
    feeds: &BTreeMap<u8, i32>,
    only: Option<u8>,
) -> Result<OutgoingPacket, MinimumError> {
    packet(Message::PetFeedList(wire::PetFeedList {
        feeds: Some(
            feeds
                .iter()
                .filter(|(t, _)| only.is_none_or(|o| o == **t))
                .map(|(t, c)| {
                    Some(PetFeedElement {
                        feed_type: *t,
                        count: *c,
                    })
                })
                .collect(),
        ),
        is_total: only.is_none(),
    }))
}
fn dialog(text: &str) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(DIALOG, text, &[])
}
fn system(text: &str, parameters: &[String]) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(SYSTEM, text, parameters)
}
/// "GameUI_Heroes_Pet_Fail_" + PetOperationResult.
fn fail_dialog(result: PetResult) -> Result<OutgoingPacket, MinimumError> {
    dialog(&format!("GameUI_Heroes_Pet_Fail_{}", result.name()))
}
fn current(pets: &[Pet]) -> Option<&Pet> {
    pets.iter().find(|p| p.status == status::SELECTED)
}

/// A pet operation's frontend messages and, when PetManager.Sync sees the current pet
/// changed, its new PetStatusInfo (CharacterSummary.Pet, PushUpdatePetStatus).
#[derive(Default)]
pub struct Outcome {
    pub packets: Vec<OutgoingPacket>,
    pub current: Option<Option<PetStatusInfo>>,
}

/// PetManager.Sync: the saved pets, the current pet's summary when it changed and
/// PushPetList (the total list when an updated pet left).
fn sync(
    pets: &[Pet],
    updated: &[i64],
    current_updated: bool,
    contents: &Contents,
    now: i64,
    out: &mut Outcome,
) -> Result<(), MinimumError> {
    let current_pet = current(pets);
    if current_updated || current_pet.is_some_and(|c| updated.contains(&c.id)) {
        out.current = Some(current_pet.map(|c| status_info(c, contents, now)));
    }
    if updated.is_empty() {
        return Ok(());
    }
    let mut listed = Vec::new();
    let mut removed = false;
    for id in updated {
        match pets.iter().find(|p| p.id == *id) {
            Some(p) => {
                if !listed.iter().any(|l: &&Pet| l.id == p.id) {
                    listed.push(p)
                }
            }
            None => removed = true,
        }
    }
    if removed {
        let all: Vec<&Pet> = pets.iter().collect();
        out.packets.push(list_packet(&all, contents, true, now)?);
    } else if !listed.is_empty() {
        out.packets
            .push(list_packet(&listed, contents, false, now)?);
    }
    Ok(())
}

/// AutoPetFeed messages: one Auto_Feed system message per food, the shared food list
/// (total when several foods were used or one ran out) and Auto_Feed_Exhause when no food
/// is left.
fn feed_messages(
    pet: &Pet,
    fed: &[rules::Fed],
    after: &BTreeMap<u8, i32>,
    contents: &Contents,
    out: &mut Vec<OutgoingPacket>,
) -> Result<(), MinimumError> {
    let Some(last) = fed.last() else {
        return Ok(());
    };
    let mut total = fed.len() > 1;
    for f in fed {
        let class = contents.feed_class(f.feed_type).ok_or_else(fail)?;
        if !after.contains_key(&f.feed_type) {
            total = true;
        }
        out.push(system(
            "GameUI_Heroes_Pet_Food_Auto_Feed",
            &[
                class.to_owned(),
                pet.name.clone(),
                f.count.to_string(),
                f.hunger.to_string(),
            ],
        )?);
    }
    out.push(feed_packet(after, (!total).then_some(last.feed_type))?);
    if after.is_empty() {
        out.push(system("GameUI_Heroes_Pet_Food_Auto_Feed_Exhause", &[])?);
    }
    Ok(())
}
async fn auto_feed_ledger(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    fed: &[rules::Fed],
    contents: &Contents,
) -> Result<(), MinimumError> {
    for f in fed {
        let class = contents.feed_class(f.feed_type).unwrap_or_default();
        ledger(
            tx,
            0,
            uid,
            "AutoFeed",
            PetResult::Success,
            class,
            &f.count.to_string(),
        )
        .await?;
    }
    Ok(())
}

/// DailyActiveTimeResetProcess over loaded pets: the reset pets and the next period start.
fn reset_periods(pets: &mut [Pet], contents: &Contents, now: i64) -> (Vec<i64>, Option<i64>) {
    let mut changed = Vec::new();
    let mut next: Option<i64> = None;
    for pet in pets.iter_mut() {
        let Some(period) = contents.period(pet.pet_type) else {
            continue;
        };
        let Some(start) = pet.deactive_start_ticks else {
            continue;
        };
        let (reset, at) = rules::period_reset(period, start, now);
        next = Some(next.map_or(at, |n| n.min(at)));
        if reset && let Some(info) = contents.info(pet.pet_type) {
            rules::reset_active_time(pet, info, now);
            changed.push(pet.id);
        }
    }
    (changed, next)
}
async fn reset_ledger(
    tx: &mut Transaction<'_, Postgres>,
    pets: &[Pet],
    changed: &[i64],
) -> Result<(), MinimumError> {
    for pet in pets.iter().filter(|p| changed.contains(&p.id)) {
        save(tx, pet).await?;
        ledger(
            tx,
            pet.id,
            pet.owner_uid,
            "ResetActiveTime",
            PetResult::Success,
            "",
            &pet.remain_active_time.unwrap_or(0).to_string(),
        )
        .await?;
    }
    Ok(())
}

/// The selected character's current pet (PetManager.CurrentPetStatusInfo) for its summary.
pub(super) async fn current_info(
    tx: &mut Transaction<'_, Postgres>,
    uid: i32,
    assets: &Assets,
) -> Result<Option<PetStatusInfo>, MinimumError> {
    let Some(contents) = assets.pets() else {
        return Ok(None);
    };
    let pets = load(tx, uid, false).await?;
    let now = now()?;
    Ok(current(&pets).map(|p| status_info(p, contents, now)))
}

/// PetManager.Load at the character's entry: the period reset, PushPetTotalList and, with
/// Pet_food_V2, PushPetFeedTotalList. Also returns the next period start (ticks) for the
/// scheduled reset.
pub async fn enter(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<(Vec<OutgoingPacket>, Option<i64>), MinimumError> {
    let Some(contents) = assets.pets() else {
        return Ok((Vec::new(), None));
    };
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let (changed, next) = reset_periods(&mut pets, contents, now);
    reset_ledger(&mut tx, &pets, &changed).await?;
    let all: Vec<&Pet> = pets.iter().collect();
    let mut packets = vec![list_packet(&all, contents, true, now)?];
    if settings.food_v2 {
        packets.push(feed_packet(&feeds(&mut tx, owner.uid).await?, None)?);
    }
    tx.commit().await?;
    Ok((packets, next))
}

/// The scheduled DailyActiveTimeResetProcess: Sync and PushPetTotalList when a pet reset.
pub async fn scheduled_reset(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<(Outcome, Option<i64>), MinimumError> {
    let contents = contents(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let (changed, next) = reset_periods(&mut pets, contents, now);
    let mut out = Outcome::default();
    if !changed.is_empty() {
        reset_ledger(&mut tx, &pets, &changed).await?;
        if current(&pets).is_some_and(|c| changed.contains(&c.id)) {
            out.current = Some(current(&pets).map(|c| status_info(c, contents, now)));
        }
        let all: Vec<&Pet> = pets.iter().collect();
        out.packets.push(list_packet(&all, contents, true, now)?);
    }
    tx.commit().await?;
    Ok((out, next))
}

/// QueryPetListProcessor: PushPetTotalList.
pub async fn query_list(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = contents(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let pets = load(&mut tx, owner.uid, false).await?;
    tx.commit().await?;
    let all: Vec<&Pet> = pets.iter().collect();
    Ok(vec![list_packet(&all, contents, true, now()?)?])
}

/// PetNameCheckProcessor: CheckPetName against the account's pets.
pub async fn check_name(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    name: Option<&str>,
) -> Result<PetResult, MinimumError> {
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let pets = load(&mut tx, owner.uid, false).await?;
    tx.commit().await?;
    Ok(match name {
        Some(name) => rules::check_name(name, &settings, &pets),
        // Regex.IsMatch(null) throws; the check fails.
        None => PetResult::CreateAgainstNamingRule,
    })
}

/// The used item: its class, count and ItemClassInfo constraint.
struct UsedItem {
    id: i64,
    class: String,
    count: i32,
}
async fn used_item(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
) -> Result<Option<UsedItem>, MinimumError> {
    sqlx::query("LOCK TABLE game_items,game_equippables IN SHARE ROW EXCLUSIVE MODE")
        .execute(&mut **tx)
        .await?;
    let snapshot =
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None).await?;
    Ok(snapshot
        .slots
        .as_ref()
        .ok_or_else(fail)?
        .iter()
        .flatten()
        .find(|s| s.id == item_id)
        .map(|s| UsedItem {
            id: s.id,
            class: s.class.clone().unwrap_or_default(),
            count: s.count,
        }))
}
/// ItemCommunicator.IsConstraintSatisfied of the used item: the failure dialog, if any.
async fn constraint(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    class: &str,
) -> Result<Option<OutgoingPacket>, MinimumError> {
    let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let indexed = assets.stats_index.class(Some(&key)).ok_or_else(fail)?;
    let economy = assets.economy.as_ref().ok_or_else(fail)?;
    let info = economy.get(indexed.source_index).ok_or_else(fail)?;
    super::minimum_use::constraint_failure(tx, owner, assets, cid, &info.constraint)
        .await?
        .map(super::minimum_equip::item_fail)
        .transpose()
}
/// UseUseableItem(1): DestroyItem and Save; the UpdateInventoryInfo of the used item.
async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item: &UsedItem,
    count: i32,
) -> Result<OutgoingPacket, MinimumError> {
    let deleted = item.count <= count;
    if deleted {
        sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1")
            .bind(item.id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM game_equippables WHERE id=$1")
            .bind(item.id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM game_items WHERE id=$1 AND owner_id=$2")
            .bind(item.id)
            .bind(cid)
            .execute(&mut **tx)
            .await?;
    } else {
        sqlx::query("UPDATE game_items SET count=count-$3 WHERE id=$1 AND owner_id=$2")
            .bind(item.id)
            .bind(cid)
            .bind(count)
            .execute(&mut **tx)
            .await?;
    }
    let slot: Slot = if deleted {
        super::minimum_move::deleted_slot(item.id)
    } else {
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .as_ref()
            .and_then(|s| s.iter().flatten().find(|s| s.id == item.id))
            .cloned()
            .ok_or_else(fail)?
    };
    let kind = inventory_wire::Kind::UpdateInventoryInfo;
    Ok(OutgoingPacket {
        category: kind.category(),
        body: inventory_wire::encode(
            kind,
            Some(&inventory_wire::Message::UpdateInventoryInfo(Some(vec![
                Some(slot),
            ]))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    })
}

/// PetCreateMessage → UseInventoryItem(ItemID, PetName), branch pet_create_: AddPet with
/// the PetInfo of the item's ExtraData. A failure sends GameUI_Heroes_Pet_Fail_<result>
/// and keeps the item.
pub async fn create(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    name: Option<&str>,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut out = Outcome::default();
    let Some(item) = used_item(&mut tx, owner, assets, cid, item_id).await? else {
        return Ok(out);
    };
    if !item.class.starts_with("pet_create_") {
        return Ok(out);
    }
    if let Some(failure) = constraint(&mut tx, owner, assets, cid, &item.class).await? {
        out.packets.push(failure);
        return Ok(out);
    }
    let Some(pet_type) = contents.extra_int(&item.class) else {
        return Ok(out);
    };
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let checked = if !rules::purchase_enabled(&settings, pets.len()) {
        Err(PetResult::CreateNotPurchaseEnable)
    } else {
        match name.map(|n| rules::check_name(n, &settings, &pets)) {
            None => Err(PetResult::CreateAgainstNamingRule),
            Some(PetResult::Success) => match (contents.info(pet_type), rules::empty_slot(&pets)) {
                (None, _) => Err(PetResult::CreateNoPetInfo),
                (Some(_), None) => Err(PetResult::CreateAllocateFail),
                (Some(info), Some(slot)) => Ok((info, slot)),
            },
            Some(failure) => Err(failure),
        }
    };
    let name = name.unwrap_or_default();
    let (info, slot) = match checked {
        Ok(c) => c,
        Err(result) => {
            ledger(
                &mut tx,
                0,
                owner.uid,
                "CreatePet",
                result,
                &pet_type.to_string(),
                name,
            )
            .await?;
            tx.commit().await?;
            out.packets.push(fail_dialog(result)?);
            return Ok(out);
        }
    };
    out.packets
        .push(consume(&mut tx, owner, assets, cid, &item, 1).await?);
    let pet = Pet::new(entity_id()?, owner.uid, name.to_owned(), info, slot, now);
    insert(&mut tx, &pet).await?;
    ledger(
        &mut tx,
        pet.id,
        owner.uid,
        "CreatePet",
        PetResult::Success,
        &pet.pet_type.to_string(),
        &pet.name,
    )
    .await?;
    let id = pet.id;
    pets.push(pet);
    // PushAddPet runs DailyActiveTimeResetProcess (a new pet starts in the current period).
    let (changed, _) = reset_periods(&mut pets, contents, now);
    reset_ledger(&mut tx, &pets, &changed).await?;
    let mut updated = vec![id];
    updated.extend(changed);
    sync(&pets, &updated, false, contents, now, &mut out)?;
    tx.commit().await?;
    Ok(out)
}

/// PetChangeNameMessage → UseInventoryItem(ItemID, PetID, PetName), branch pet_name_change:
/// ChangeName, then PetChangeNameResultMessage(success, name, result).
pub async fn change_name(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    pet_id: i64,
    name: Option<&str>,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut out = Outcome::default();
    let Some(item) = used_item(&mut tx, owner, assets, cid, item_id).await? else {
        return Ok(out);
    };
    if !item.class.starts_with("pet_name_change") {
        return Ok(out);
    }
    if let Some(failure) = constraint(&mut tx, owner, assets, cid, &item.class).await? {
        out.packets.push(failure);
        return Ok(out);
    }
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let result = match (pets.iter().position(|p| p.id == pet_id), name) {
        (None, _) => PetResult::ChangeNameHasNotPet,
        (Some(_), None) => PetResult::CreateAgainstNamingRule,
        (Some(index), Some(name)) => {
            let checked = rules::check_name(name, &settings, &pets);
            if checked == PetResult::Success {
                let before = std::mem::replace(&mut pets[index].name, name.to_owned());
                save(&mut tx, &pets[index]).await?;
                ledger(
                    &mut tx,
                    pet_id,
                    owner.uid,
                    "ChangeName",
                    checked,
                    &before,
                    name,
                )
                .await?;
                out.packets
                    .push(consume(&mut tx, owner, assets, cid, &item, 1).await?);
                sync(&pets, &[pet_id], false, contents, now, &mut out)?;
            }
            checked
        }
    };
    tx.commit().await?;
    out.packets.push(packet(Message::PetChangeNameResult(
        wire::PetChangeNameResult {
            is_success: result == PetResult::Success,
            pet_name: name.map(str::to_owned),
            result_type: Some(result.name().to_owned()),
        },
    ))?);
    Ok(out)
}

/// SelectPetProcessor: SelectPet(0) recalls the current pet; another pet becomes the current
/// one (the previous returns to Useabled) and, with Pet_food_V2, eats the shared food.
/// Disabled pets can be selected, as in the source.
pub async fn select(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    pet_id: i64,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let mut out = Outcome::default();
    let mut updated = Vec::new();
    let previous = pets.iter().position(|p| p.status == status::SELECTED);
    if pet_id == 0 {
        let Some(index) = previous else {
            return Ok(out);
        };
        pets[index].status = status::USEABLE;
        rules::update_status(&mut pets[index], now);
        updated.push(pets[index].id);
    } else {
        let Some(index) = pets.iter().position(|p| p.id == pet_id) else {
            return Ok(out);
        };
        if let Some(p) = previous {
            pets[p].status = status::USEABLE;
            updated.push(pets[p].id);
        }
        pets[index].status = status::SELECTED;
        if settings.food_v2
            && let Some(info) = contents.info(pets[index].pet_type)
        {
            let before = feeds(&mut tx, owner.uid).await?;
            let mut shared = before.clone();
            let fed = rules::auto_feed(&mut pets[index], info, &mut shared, contents);
            store_feeds(&mut tx, owner.uid, &before, &shared).await?;
            auto_feed_ledger(&mut tx, owner.uid, &fed, contents).await?;
            feed_messages(&pets[index], &fed, &shared, contents, &mut out.packets)?;
        }
        updated.push(pets[index].id);
    }
    for pet in pets.iter().filter(|p| updated.contains(&p.id)) {
        save(&mut tx, pet).await?;
    }
    sync(&pets, &updated, true, contents, now, &mut out)?;
    tx.commit().await?;
    Ok(out)
}

/// MovePetSlotProcessor: SwapSlot within PetPurchaseLimit.
pub async fn move_slot(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    pet_id: i64,
    dest: i32,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let mut out = Outcome::default();
    let Some(changed) = rules::swap_slot(&mut pets, pet_id, dest, &settings) else {
        return Ok(out);
    };
    sqlx::query("SET CONSTRAINTS game_pets_owner_slot DEFERRED")
        .execute(&mut *tx)
        .await?;
    for pet in pets.iter().filter(|p| changed.contains(&p.id)) {
        save(&mut tx, pet).await?;
    }
    sync(&pets, &changed, false, contents, now()?, &mut out)?;
    tx.commit().await?;
    Ok(out)
}

/// RemovePetProcessor: the pet and its cash skills are deleted.
pub async fn remove(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    pet_id: i64,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let mut out = Outcome::default();
    let Some(index) = pets.iter().position(|p| p.id == pet_id) else {
        return Ok(out);
    };
    let pet = pets.remove(index);
    sqlx::query("DELETE FROM game_pets WHERE id=$1 AND owner_uid=$2")
        .bind(pet.id)
        .bind(owner.uid)
        .execute(&mut *tx)
        .await?;
    ledger(
        &mut tx,
        pet.id,
        owner.uid,
        "RemovePet",
        PetResult::Success,
        &pet.pet_type.to_string(),
        &pet.name,
    )
    .await?;
    sync(
        &pets,
        &[pet.id],
        pet.status == status::SELECTED,
        contents,
        now()?,
        &mut out,
    )?;
    tx.commit().await?;
    Ok(out)
}

/// RemovePetSkillProcessor: a cash skill whose validity has passed; the system message of
/// the frontend's OnComplete/OnFail follows.
pub async fn remove_skill(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    pet_id: i64,
    skill_id: i32,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let mut out = Outcome::default();
    let result = match pets.iter_mut().find(|p| p.id == pet_id) {
        None => PetResult::RemoveSkillNoPetInfo,
        Some(pet) => match pet.skills.iter().position(|s| s.skill_id == skill_id) {
            None => PetResult::RemoveSkillNotHasSkill,
            Some(i) if pet.skills[i].expired_ticks.is_some_and(|e| e > now) => {
                PetResult::RemoveSkillExpiredTimeRemain
            }
            Some(i) => {
                let skill = pet.skills.remove(i);
                sqlx::query("DELETE FROM game_pet_skills WHERE row_id=$1 AND pet_id=$2")
                    .bind(skill.row_id)
                    .bind(pet_id)
                    .execute(&mut *tx)
                    .await?;
                PetResult::Success
            }
        },
    };
    let ledger_pet = if result == PetResult::RemoveSkillNoPetInfo {
        0
    } else {
        pet_id
    };
    ledger(
        &mut tx,
        ledger_pet,
        owner.uid,
        "RemoveCashSkill",
        result,
        &pet_id.to_string(),
        &skill_id.to_string(),
    )
    .await?;
    if result == PetResult::Success {
        sync(&pets, &[pet_id], false, contents, now, &mut out)?;
    }
    tx.commit().await?;
    out.packets.push(system(
        if result == PetResult::Success {
            "#GameUI_Heroes_Pet_Success_RemoveSkill"
        } else {
            "#GameUI_Heroes_Pet_Fail_RemoveSkill"
        },
        &[],
    )?);
    Ok(out)
}

/// UseInventoryItemProcessor pet_ branch for UseInventoryItemMessage, inside the use
/// transaction: pet_feed_ (FeedCurrentPet with the item's ExtraData) and pet_skill_
/// (GiveCashSkill with the validity in the item name). pet_create_ and pet_name_change
/// need a name and come from their own messages; other pet_ items fail without a message.
pub(super) async fn use_item(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
    class: &str,
    count: i32,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let mut out = Outcome::default();
    let feed = class.starts_with("pet_feed_");
    if !feed && !class.starts_with("pet_skill_") {
        return Ok(out);
    }
    if let Some(failure) = constraint(tx, owner, assets, cid, class).await? {
        out.packets.push(failure);
        return Ok(out);
    }
    let Some(value) = contents.extra_int(class) else {
        return Ok(out);
    };
    let now = now()?;
    let mut pets = load(tx, owner.uid, true).await?;
    let index = pets.iter().position(|p| p.status == status::SELECTED);
    let item = UsedItem {
        id: item_id,
        class: class.to_owned(),
        count,
    };
    let result = if feed {
        match index {
            None => PetResult::FeedNoSelectedPet,
            Some(i) => match contents.info(pets[i].pet_type) {
                None => return Err(fail()),
                Some(info) if pets[i].hunger >= info.max_hunger => PetResult::FeedAlreadyMaxHunger,
                Some(info) => {
                    if rules::feed(&mut pets[i], value, info) {
                        save(tx, &pets[i]).await?;
                        ledger(
                            tx,
                            pets[i].id,
                            owner.uid,
                            "FeedPet",
                            PetResult::Success,
                            &value.to_string(),
                            &pets[i].hunger.to_string(),
                        )
                        .await?;
                    }
                    PetResult::Success
                }
            },
        }
    } else {
        match index {
            None => PetResult::GiveSkillNoSelectedPet,
            Some(i) => match rules::check_cash_skill(&pets[i], value, contents) {
                Err(failure) => failure,
                Ok(slot) => {
                    let info = contents.skill(value).ok_or_else(fail)?;
                    let skill = PetSkill {
                        row_id: entity_id()?,
                        slot,
                        skill_id: value,
                        open_level: 1,
                        cool_time_type: u8::try_from(info.cool_time_type.ok_or_else(fail)?)
                            .map_err(|_| fail())?,
                        expired_ticks: rules::item_validity_ticks(class).map(|t| now + t),
                    };
                    sqlx::query("INSERT INTO game_pet_skills(row_id,pet_id,slot,skill_id,open_level,cool_time_type,cool_time,last_use_ticks,expired_ticks) VALUES($1,$2,$3,$4,$5,$6,0,$7,$8)")
                        .bind(skill.row_id)
                        .bind(pets[i].id)
                        .bind(skill.slot)
                        .bind(skill.skill_id)
                        .bind(skill.open_level)
                        .bind(i16::from(skill.cool_time_type))
                        .bind(now)
                        .bind(skill.expired_ticks)
                        .execute(&mut **tx)
                        .await?;
                    ledger(
                        tx,
                        pets[i].id,
                        owner.uid,
                        "AddNewCashSkill",
                        PetResult::Success,
                        &skill.row_id.to_string(),
                        &skill.slot.to_string(),
                    )
                    .await?;
                    pets[i].skills.push(skill);
                    PetResult::Success
                }
            },
        }
    };
    if result != PetResult::Success {
        let pet = index.map_or(0, |i| pets[i].id);
        let operation = if feed { "FeedPet" } else { "AddNewCashSkill" };
        ledger(
            tx,
            pet,
            owner.uid,
            operation,
            result,
            &value.to_string(),
            "",
        )
        .await?;
        out.packets.push(fail_dialog(result)?);
        return Ok(out);
    }
    out.packets
        .push(consume(tx, owner, assets, cid, &item, 1).await?);
    let id = index.map(|i| pets[i].id).ok_or_else(fail)?;
    sync(&pets, &[id], false, contents, now, &mut out)?;
    Ok(out)
}

/// RequestPetFoodShareProcessor: the whole stack becomes the account's shared food of its
/// FeedType; the current pet then eats from it.
pub async fn food_share(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_id: i64,
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut out = Outcome::default();
    let Some(item) = used_item(&mut tx, owner, assets, cid, item_id).await? else {
        out.packets
            .push(dialog("GameUI_Heroes_Pet_Fail_Food_Share")?);
        return Ok(out);
    };
    let feed_type = contents.feed_type(&item.class);
    if feed_type == 0 {
        out.packets
            .push(dialog("GameUI_Heroes_Pet_Fail_Food_Share")?);
        return Ok(out);
    }
    out.packets
        .push(consume(&mut tx, owner, assets, cid, &item, item.count).await?);
    let before = feeds(&mut tx, owner.uid).await?;
    let mut shared = before.clone();
    let total = shared
        .get(&feed_type)
        .copied()
        .unwrap_or(0)
        .checked_add(item.count)
        .ok_or_else(fail)?;
    set_feed(&mut tx, owner.uid, feed_type, total).await?;
    shared.insert(feed_type, total);
    ledger(
        &mut tx,
        0,
        owner.uid,
        "FoodShare",
        PetResult::Success,
        &item.class,
        &item.count.to_string(),
    )
    .await?;
    out.packets.push(feed_packet(&shared, Some(feed_type))?);
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    if let Some(index) = pets.iter().position(|p| p.status == status::SELECTED)
        && let Some(info) = contents.info(pets[index].pet_type)
    {
        let before = shared.clone();
        let fed = rules::auto_feed(&mut pets[index], info, &mut shared, contents);
        if !fed.is_empty() {
            store_feeds(&mut tx, owner.uid, &before, &shared).await?;
            auto_feed_ledger(&mut tx, owner.uid, &fed, contents).await?;
            feed_messages(&pets[index], &fed, &shared, contents, &mut out.packets)?;
            save(&mut tx, &pets[index]).await?;
            let id = pets[index].id;
            sync(&pets, &[id], false, contents, now, &mut out)?;
        }
    }
    tx.commit().await?;
    Ok(out)
}

/// RequestPetFoodUnshareProcessor: the shared food of the class returns as a
/// character-bound item. The source mails it when the inventory is full (mail is not
/// migrated): then the food stays shared and the failure dialog is sent.
pub async fn food_unshare(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    item_class: Option<&str>,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<Outcome, MinimumError> {
    let contents = contents(assets)?;
    let mut out = Outcome::default();
    let refused = || dialog("GameUI_Heroes_Pet_Fail_Food_Unshare");
    let class = item_class.unwrap_or_default();
    let feed_type = contents.feed_type(class);
    if feed_type == 0 {
        out.packets.push(refused()?);
        return Ok(out);
    }
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut shared = feeds(&mut tx, owner.uid).await?;
    let Some(count) = shared.get(&feed_type).copied() else {
        out.packets.push(refused()?);
        return Ok(out);
    };
    let key: Vec<u8> = class.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let attributes = crate::domain::item_attribute::Attributes::default();
    let request = super::minimum_give::Request {
        class: &key,
        count,
        bound: true,
        equipment: None,
        attributes: &attributes,
    };
    let mut next = || ids.next().ok_or_else(fail);
    let applied = match super::minimum_give::apply(
        &mut tx,
        owner,
        assets,
        cid,
        &[],
        std::slice::from_ref(&request),
        &mut next,
    )
    .await?
    {
        Ok(a) => a,
        Err(_) => {
            ledger(
                &mut tx,
                0,
                owner.uid,
                "FoodUnshare",
                PetResult::FeedNoEmptySlot,
                class,
                "",
            )
            .await?;
            tx.commit().await?;
            out.packets.push(refused()?);
            return Ok(out);
        }
    };
    set_feed(&mut tx, owner.uid, feed_type, 0).await?;
    shared.remove(&feed_type);
    ledger(
        &mut tx,
        0,
        owner.uid,
        "FoodUnshare",
        PetResult::Success,
        class,
        "",
    )
    .await?;
    tx.commit().await?;
    let kind = inventory_wire::Kind::UpdateInventoryInfo;
    out.packets.push(OutgoingPacket {
        category: kind.category(),
        body: inventory_wire::encode(
            kind,
            Some(&inventory_wire::Message::UpdateInventoryInfo(Some(
                applied.slots,
            ))),
            DEFAULT_MAX_BODY,
        )
        .map_err(|_| fail())?
        .to_vec(),
    });
    out.packets.push(feed_packet(&shared, None)?);
    Ok(out)
}

/// RewardPetElement of MicroPlayerPetManager.EndBattle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RewardPet {
    pub pet_id: i64,
    pub play_time: i32,
    pub exp: i32,
}

/// RewardPetEndBattleProcessor: UpdatePetAfterBattle of each listed pet, then Sync.
pub async fn reward_end_battle(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    rewards: &[RewardPet],
) -> Result<Outcome, MinimumError> {
    let mut out = Outcome::default();
    if rewards.is_empty() {
        return Ok(out);
    }
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut pets = load(&mut tx, owner.uid, true).await?;
    let now = now()?;
    let before = feeds(&mut tx, owner.uid).await?;
    let mut shared = before.clone();
    let mut updated = Vec::new();
    for reward in rewards {
        let Some(index) = pets.iter().position(|p| p.id == reward.pet_id) else {
            continue;
        };
        let pet = &mut pets[index];
        let food = settings.food_v2.then_some(&mut shared);
        let Some(battle) = rules::after_battle(
            pet,
            reward.play_time,
            reward.exp,
            &settings,
            contents,
            food,
            now,
        ) else {
            continue;
        };
        let pet = &pets[index];
        if let Some(gained) = battle.exp_gained {
            ledger(
                &mut tx,
                pet.id,
                owner.uid,
                "GiveExp",
                PetResult::Success,
                &pet.level.to_string(),
                &gained.to_string(),
            )
            .await?;
        }
        ledger(
            &mut tx,
            pet.id,
            owner.uid,
            "DecreaseHungerDesire",
            PetResult::Success,
            &battle.hunger_decrease.to_string(),
            &pet.hunger.to_string(),
        )
        .await?;
        auto_feed_ledger(&mut tx, owner.uid, &battle.fed, contents).await?;
        feed_messages(pet, &battle.fed, &shared, contents, &mut out.packets)?;
        ledger(
            &mut tx,
            pet.id,
            owner.uid,
            "DecreaseActiveTime",
            PetResult::Success,
            &reward.play_time.to_string(),
            &pet.remain_active_time.unwrap_or(0).to_string(),
        )
        .await?;
        save(&mut tx, pet).await?;
        updated.push(pet.id);
    }
    store_feeds(&mut tx, owner.uid, &before, &shared).await?;
    sync(&pets, &updated, false, contents, now, &mut out)?;
    tx.commit().await?;
    Ok(out)
}

/// A pet of MicroPlayerPetManager.UsePetList.
struct BattlePet {
    info: PetStatusInfo,
    added_exp: f64,
    start: Option<std::time::Instant>,
    spend: i64,
    alive: bool,
}
impl BattlePet {
    fn start(&mut self, now: std::time::Instant) {
        if self.start.is_none() {
            self.start = Some(now);
        }
    }
    /// EndUsingPet: whole seconds of use are added.
    fn end(&mut self, now: std::time::Instant) {
        if let Some(start) = self.start.take() {
            self.spend +=
                i64::try_from(now.saturating_duration_since(start).as_secs()).unwrap_or(i64::MAX);
        }
    }
}
/// MicroPlayerPetManager of the mission's player: the current pet, the pets used in the
/// battle with their play time and experience (Pet_ExpBattle_Point % of the player's
/// monster experience), and the pet death/revival of the host.
pub struct BattlePets {
    current: Option<PetStatusInfo>,
    used: Vec<BattlePet>,
    rate: f64,
    /// The summary pet after RewardPetEndBattle changed the current pet, for the frontend.
    summary: Option<Option<PetStatusInfo>>,
}
impl BattlePets {
    pub fn new(assets: &Assets) -> Self {
        Self {
            current: None,
            used: Vec::new(),
            rate: settings(assets).map_or(0.0, |s| s.exp_battle_rate),
            summary: None,
        }
    }
    pub fn set_summary(&mut self, summary: Option<Option<PetStatusInfo>>) {
        if summary.is_some() {
            self.summary = summary;
        }
    }
    /// The pending summary pet change (PushUpdatePetStatus after the battle reward).
    pub fn take_summary(&mut self) -> Option<Option<PetStatusInfo>> {
        self.summary.take()
    }
    pub fn current(&self) -> Option<&PetStatusInfo> {
        self.current.as_ref()
    }
    fn current_index(&self) -> Option<usize> {
        let id = self.current.as_ref()?.pet_id;
        self.used.iter().position(|p| p.info.pet_id == id)
    }
    fn add_used(&mut self, pet: &PetStatusInfo) {
        if !self.used.iter().any(|p| p.info.pet_id == pet.pet_id) {
            self.used.push(BattlePet {
                info: pet.clone(),
                added_exp: 0.0,
                start: None,
                spend: 0,
                alive: true,
            });
        }
    }
    /// SetForceCurrentPet: no current pet without active time left.
    pub fn set_force_current(&mut self, pet: Option<PetStatusInfo>) {
        self.current = pet.filter(|p| p.remain_active_time > 0);
    }
    /// ChangeCurrentPet (a selection while InGame): the previous pet stops counting.
    pub fn change_current(&mut self, pet: Option<PetStatusInfo>, now: std::time::Instant) {
        if let Some(i) = self.current_index() {
            self.used[i].end(now);
            self.current = None;
        }
        let Some(pet) = pet.filter(|p| p.pet_id != 0 && p.remain_active_time > 0) else {
            self.current = None;
            return;
        };
        self.add_used(&pet);
        self.current = Some(pet);
        if let Some(i) = self.current_index() {
            self.used[i].start(now);
        }
    }
    /// StartBattle.
    pub fn start_battle(&mut self, now: std::time::Instant) {
        if let Some(pet) = self.current.clone()
            && pet.remain_active_time > 0
        {
            self.add_used(&pet);
        }
        if let Some(i) = self.current_index() {
            self.used[i].start(now);
        }
    }
    /// GiveExpToCurrentPet (the player's monster kill experience).
    pub fn give_exp(&mut self, exp: i32) {
        let rate = self.rate;
        if let Some(i) = self.current_index()
            && self.used[i].start.is_some()
        {
            self.used[i].added_exp += f64::from(exp) * rate;
        }
    }
    /// PetKilledProcessor: the current pet with this PetID stops counting.
    pub fn killed(&mut self, pet_id: i64, now: std::time::Instant) -> bool {
        let Some(i) = self.current_index() else {
            return false;
        };
        let pet = &mut self.used[i];
        if pet.info.pet_id != pet_id || !pet.alive {
            return false;
        }
        pet.alive = false;
        pet.end(now);
        true
    }
    /// PetRevivedProcessor: the current pet with this PetID counts again.
    pub fn revived(&mut self, pet_id: i64, now: std::time::Instant) -> bool {
        let Some(i) = self.current_index() else {
            return false;
        };
        let pet = &mut self.used[i];
        if pet.info.pet_id != pet_id || pet.alive {
            return false;
        }
        pet.alive = true;
        pet.start(now);
        true
    }
    /// EndBattle: the RewardPetEndBattle list (experience only with `include_exp`), then
    /// Clear.
    pub fn end_battle(&mut self, include_exp: bool, now: std::time::Instant) -> Vec<RewardPet> {
        if let Some(i) = self.current_index() {
            self.used[i].end(now);
        }
        let rewards = self
            .used
            .iter()
            .map(|p| RewardPet {
                pet_id: p.info.pet_id,
                play_time: i32::try_from(p.spend).unwrap_or(i32::MAX),
                exp: if include_exp { p.added_exp as i32 } else { 0 },
            })
            .collect();
        self.current = None;
        self.used.clear();
        rewards
    }
}
/// UpdateCurrentPetMessage to the host (UID = the member's slot).
pub fn update_current_packet(
    slot: i32,
    pet: Option<&PetStatusInfo>,
) -> Result<OutgoingPacket, MinimumError> {
    packet(Message::UpdateCurrentPet(wire::UpdateCurrentPet {
        uid: slot,
        current_pet: pet.cloned(),
    }))
}
/// The name check answers of PetCreateNameCheck/PetChangeNameCheck.
pub fn create_name_answer(
    item_id: i64,
    name: Option<&str>,
    result: PetResult,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    if result != PetResult::Success {
        packets.push(fail_dialog(result)?);
    }
    packets.push(packet(Message::PetCreateNameCheckResult(
        wire::PetCreateNameCheckResult {
            item_id,
            pet_name: name.map(str::to_owned),
            is_success: result == PetResult::Success,
        },
    ))?);
    Ok(packets)
}
pub fn change_name_answer(
    item_id: i64,
    pet_id: i64,
    name: Option<&str>,
    result: PetResult,
) -> Result<OutgoingPacket, MinimumError> {
    packet(Message::PetChangeNameCheckResult(
        wire::PetChangeNameCheckResult {
            is_success: result == PetResult::Success,
            item_id,
            pet_id,
            pet_name: name.map(str::to_owned),
            result_type: Some(result.name().to_owned()),
        },
    ))
}

/// Panel view of one pet.
#[derive(Clone, Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PanelPet {
    pub id: String,
    pub name: String,
    pub pet_type: i32,
    pub item_class: Option<String>,
    pub slot: i32,
    pub status: u8,
    pub level: i32,
    pub exp: i32,
    pub hunger: i32,
    pub max_hunger: Option<i32>,
    pub remain_active_time: i32,
    pub cash_skills: Vec<i32>,
}
fn panel_pet(p: Pet, assets: &Assets) -> PanelPet {
    let info = assets.pets().and_then(|c| c.info(p.pet_type));
    PanelPet {
        id: p.id.to_string(),
        item_class: info.and_then(|i| i.item_class.clone()),
        max_hunger: info.map(|i| i.max_hunger),
        name: p.name,
        pet_type: p.pet_type,
        slot: p.slot,
        status: p.status,
        level: p.level,
        exp: p.exp,
        hunger: p.hunger,
        remain_active_time: p.remain_active_time.unwrap_or(0),
        cash_skills: p.skills.iter().map(|s| s.skill_id).collect(),
    }
}
/// The pets of an account for the panel (read only, no character lock).
pub async fn panel_list(
    pool: &PgPool,
    assets: &Assets,
    uid: i32,
) -> Result<Vec<PanelPet>, MinimumError> {
    let mut tx = pool.begin().await?;
    let pets = load(&mut tx, uid, false).await?;
    tx.commit().await?;
    Ok(pets.into_iter().map(|p| panel_pet(p, assets)).collect())
}
/// A PetInfo type for the panel.
#[derive(Clone, Debug, serde::Serialize, utoipa::ToSchema)]
pub struct PanelPetType {
    pub pet_type: i32,
    /// The pet_create_ item of the type.
    pub item_class: Option<String>,
}
/// The panel's pet types (PetInfo).
pub fn panel_types(assets: &Assets) -> Vec<PanelPetType> {
    assets.pets().map_or_else(Vec::new, |c| {
        c.infos()
            .map(|i| PanelPetType {
                pet_type: i.pet_id,
                item_class: i.item_class.clone(),
            })
            .collect()
    })
}
/// Panel refusals the operator can correct.
#[derive(Debug, PartialEq, Eq)]
pub enum PanelError {
    Online,
    Refused(PetResult),
    Unknown,
    Repeated,
}
async fn panel_audit(
    tx: &mut Transaction<'_, Postgres>,
    audit: &super::admin_store::AuditRecord<'_>,
) -> Result<bool, MinimumError> {
    match super::admin_store::insert_audit(tx, audit).await {
        Ok(()) => Ok(true),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Ok(false),
        Err(e) => Err(e.into()),
    }
}
/// The panel's AddPet (the GM _CHEAT_AddPet) for an offline account: PetManager.AddPet
/// with the same checks, audited once per operation.
pub async fn panel_add(
    pool: &PgPool,
    assets: &Assets,
    uid: i32,
    pet_type: i32,
    name: &str,
    mut audit: super::admin_store::AuditRecord<'_>,
) -> Result<Result<PanelPet, PanelError>, MinimumError> {
    let contents = contents(assets)?;
    let settings = settings(assets)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT id FROM game_users WHERE id=$1 FOR UPDATE")
        .bind(uid)
        .fetch_one(&mut *tx)
        .await?;
    if panel_online(&mut tx, uid).await? {
        return Ok(Err(PanelError::Online));
    }
    let pets = load(&mut tx, uid, true).await?;
    let checked = if !rules::purchase_enabled(&settings, pets.len()) {
        Err(PetResult::CreateNotPurchaseEnable)
    } else {
        match rules::check_name(name, &settings, &pets) {
            PetResult::Success => match (contents.info(pet_type), rules::empty_slot(&pets)) {
                (None, _) => Err(PetResult::CreateNoPetInfo),
                (Some(_), None) => Err(PetResult::CreateAllocateFail),
                (Some(info), Some(slot)) => Ok((info, slot)),
            },
            failure => Err(failure),
        }
    };
    let (info, slot) = match checked {
        Ok(c) => c,
        Err(result) => return Ok(Err(PanelError::Refused(result))),
    };
    let pet = Pet::new(entity_id()?, uid, name.to_owned(), info, slot, now()?);
    insert(&mut tx, &pet).await?;
    ledger(
        &mut tx,
        pet.id,
        uid,
        "CreatePet",
        PetResult::Success,
        &pet_type.to_string(),
        name,
    )
    .await?;
    let view = panel_pet(pet, assets);
    audit.after =
        Some(serde_json::json!({ "pet": view.id, "type": pet_type, "name": name }).to_string());
    if !panel_audit(&mut tx, &audit).await? {
        return Ok(Err(PanelError::Repeated));
    }
    tx.commit().await?;
    Ok(Ok(view))
}
/// The panel's RemovePet for an offline account, audited once per operation.
pub async fn panel_remove(
    pool: &PgPool,
    uid: i32,
    pet_id: i64,
    mut audit: super::admin_store::AuditRecord<'_>,
) -> Result<Result<(), PanelError>, MinimumError> {
    let mut tx = pool.begin().await?;
    if panel_online(&mut tx, uid).await? {
        return Ok(Err(PanelError::Online));
    }
    let removed: Option<(String, i32)> = sqlx::query_as(
        "DELETE FROM game_pets WHERE id=$1 AND owner_uid=$2 RETURNING name,pet_type",
    )
    .bind(pet_id)
    .bind(uid)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((name, pet_type)) = removed else {
        return Ok(Err(PanelError::Unknown));
    };
    ledger(
        &mut tx,
        pet_id,
        uid,
        "RemovePet",
        PetResult::Success,
        &pet_type.to_string(),
        &name,
    )
    .await?;
    audit.before = Some(
        serde_json::json!({ "pet": pet_id.to_string(), "type": pet_type, "name": name })
            .to_string(),
    );
    if !panel_audit(&mut tx, &audit).await? {
        return Ok(Err(PanelError::Repeated));
    }
    tx.commit().await?;
    Ok(Ok(()))
}
/// Whether a character of the account is in the game (the pets are its PetManager's).
async fn panel_online(tx: &mut Transaction<'_, Postgres>, uid: i32) -> Result<bool, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM game_character_info WHERE uid=$1 AND is_connected)",
    )
    .bind(uid)
    .fetch_one(&mut **tx)
    .await?)
}
/// The selected character's town look after PushUpdatePetStatus (CharacterSummary.Pet).
pub async fn look(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    config: super::character_lobby::QueryConfig,
    arithmetic: crate::domain::character_stats::Arithmetic,
) -> Result<OutgoingPacket, MinimumError> {
    let mut tx = pool.begin().await?;
    let view =
        super::minimum_selection::view(&mut tx, owner, assets, cid, config, arithmetic, false, 0)
            .await?;
    tx.commit().await?;
    Ok(view.look)
}
