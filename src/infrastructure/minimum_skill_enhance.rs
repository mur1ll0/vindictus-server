//! SkillEnhance of the selected first Fiona: dbo.SkillEnhance rows (SkillEnhanceQueryAll
//! through game_skills.cid), their BriefSkillEnhance projections and the session
//! processors (SkillEnhanceSessionStart, SessionUseErg, SkillEnhanceSession and
//! SessionEnd) with the session held by the selected character.
use super::{
    character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError,
    minimum_give::Request,
};
use crate::{
    domain::{
        item_attribute::{Attribute, Attributes},
        map_partition::OutgoingPacket,
        skill_enhance::{self, Enhance, Outcome, Session},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message, SkillEnhance as Brief, Slot},
        mission::StrDict,
    },
};
use sqlx::{PgPool, Postgres, Transaction};

/// ItemLogType.SkillEnhance/DestroyItem/GiveItem and the SkillEnhance ActionTypes.
const LOG_GIVE: i16 = 2;
const LOG_DESTROY: i16 = 3;
const ACTION_USE_ERG: i16 = 151;
const ACTION_DICE_SUCCESS: i16 = 152;
const ACTION_DICE_FAIL: i16 = 153;
/// SystemMessageCategory.System.
const SYSTEM: u8 = 0;

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect();
    String::from_utf16(&units).map_err(|_| fail())
}
fn packet(kind: Kind, message: Message) -> Result<OutgoingPacket, MinimumError> {
    Ok(OutgoingPacket {
        category: kind.category(),
        body: wire::encode(kind, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

/// The character state the processors keep between messages.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Character.SkillEnhanceSession.
    pub session: Option<Session>,
    /// SkillManager.UpdatedEnhances (never cleared by FlushChangedEnhances).
    pub updated: Vec<i64>,
}

/// SkillEnhance.ToBriefSkillEnhance.
pub(super) fn brief(e: &Enhance) -> Brief {
    Brief {
        group_key: e.group,
        index_key: e.key,
        kind: e.kind,
        reduce_durability: e.reduce_durability,
        max_durability_bonus: e.max_durability_bonus,
    }
}

/// SkillID, SkillRowID, GroupKey, IndexKey, Type, ReduceDurability, MaxDurabilityBonus.
type EnhanceRow = (Vec<u8>, i64, i32, i32, i16, i32, i32);
/// SkillEnhanceQueryAll: (SkillID, row) of the character's skills, by SkillRowID.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<(String, Enhance)>, MinimumError> {
    let rows: Vec<EnhanceRow> = sqlx::query_as(
        "SELECT s.skill_utf16,e.skill_row_id,e.group_key,e.index_key,e.type,e.reduce_durability,e.max_durability_bonus FROM game_skill_enhances e JOIN game_skills s ON s.row_id=e.skill_row_id WHERE s.cid=$1 ORDER BY e.skill_row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if rows.len() > 4096 {
        return Err(fail());
    }
    rows.into_iter()
        .map(|(skill, row, group, key, kind, reduce, bonus)| {
            Ok((
                text(&skill)?,
                Enhance {
                    skill_row_id: row,
                    group,
                    key,
                    kind: u8::try_from(kind).map_err(|_| fail())?,
                    reduce_durability: reduce,
                    max_durability_bonus: bonus,
                },
            ))
        })
        .collect()
}

/// SkillManager.GetBriefSkillEnhanceData: the first row of each skill whose Type is not
/// None (the MicroPlay member data).
pub(super) async fn brief_data(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<Vec<(String, Option<Brief>)>, MinimumError> {
    let mut out: Vec<(String, Option<Brief>)> = Vec::new();
    for (skill, e) in load(tx, cid).await? {
        if e.kind != skill_enhance::TYPE_NONE && !out.iter().any(|(s, _)| *s == skill) {
            out.push((skill, Some(brief(&e))));
        }
    }
    Ok(out)
}

fn system(category: u8, outcome: Outcome) -> Result<OutgoingPacket, MinimumError> {
    super::minimum_shop::system_message(
        category,
        "GameUI_Heroes_SkillEnhance_{0}",
        &[outcome.name().to_owned()],
    )
}

/// The character's skill (RowID) and its enhance row.
async fn skill_row(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    skill: &str,
) -> Result<Option<(i64, Option<Enhance>)>, MinimumError> {
    let row: Option<i64> =
        sqlx::query_scalar("SELECT row_id FROM game_skills WHERE cid=$1 AND skill_utf16=$2")
            .bind(cid)
            .bind(utf16(skill))
            .fetch_optional(&mut **tx)
            .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let enhance = load(tx, cid)
        .await?
        .into_iter()
        .find(|(_, e)| e.skill_row_id == row)
        .map(|(_, e)| e);
    Ok(Some((row, enhance)))
}

/// The owner's item: (ItemClass, Num, attributes).
async fn item(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    id: i64,
) -> Result<Option<(String, i32, Attributes)>, MinimumError> {
    let row: Option<(Vec<u8>, i32)> = sqlx::query_as(
        "SELECT item_class_utf16,count FROM game_items WHERE id=$1 AND owner_id=$2 AND collection<>102",
    )
    .bind(id)
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((class, count)) = row else {
        return Ok(None);
    };
    let attributes = super::minimum_inventory::load_attributes(tx, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    Ok(Some((text(&class)?, count, attributes)))
}

/// Item.Num -= n of one item (the last units delete it with its attributes).
async fn consume(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    id: i64,
    n: i32,
) -> Result<(), MinimumError> {
    let count: i32 = sqlx::query_scalar("SELECT count FROM game_items WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    if count <= n {
        super::minimum_enhance::delete_item(tx, cid, id).await
    } else {
        sqlx::query("UPDATE game_items SET count=count-$3 WHERE id=$1 AND owner_id=$2")
            .bind(id)
            .bind(cid)
            .bind(n)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }
}

/// UpdateInventoryInfo of changed item IDs (a missing item is a deleted SlotInfo).
async fn slots_packet(
    tx: &mut Transaction<'_, Postgres>,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    ids: &[i64],
    extra: Vec<Option<Slot>>,
) -> Result<OutgoingPacket, MinimumError> {
    let slots: Vec<Slot> =
        super::minimum_inventory::snapshot_with_equipment(tx, owner, assets, cid, None)
            .await?
            .slots
            .ok_or_else(fail)?
            .into_iter()
            .flatten()
            .collect();
    let mut changed: Vec<Option<Slot>> = ids
        .iter()
        .map(|id| {
            Some(
                slots
                    .iter()
                    .find(|s| s.id == *id)
                    .cloned()
                    .unwrap_or_else(|| super::minimum_move::deleted_slot(*id)),
            )
        })
        .collect();
    changed.extend(extra);
    packet(
        Kind::UpdateInventoryInfo,
        Message::UpdateInventoryInfo(Some(changed)),
    )
}

/// The selected character's connected transaction with the item tables locked.
async fn begin<'a>(
    pool: &'a PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<Transaction<'a, Postgres>, MinimumError> {
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
    super::minimum_inventory::snapshot_with_equipment(&mut tx, owner, assets, cid, None).await?;
    Ok(tx)
}

/// SkillEnhanceSessionStartProcessor: the session is replaced (null on a refusal, which
/// answers its SystemMessage).
pub async fn start(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    state: &mut State,
    request: &wire::SkillEnhanceSessionStart,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let policy = contents.policy().ok_or_else(fail)?;
    // GetSkill(null) throws in the skill dictionary.
    let name = request.skill_name.as_deref().ok_or_else(fail)?;
    let mut tx = begin(pool, owner, assets, cid).await?;
    let skill = skill_row(&mut tx, cid, name).await?;
    let stone = item(&mut tx, cid, request.stone_item_id).await?;
    tx.commit().await?;
    let info = stone
        .as_ref()
        .and_then(|(class, _, _)| contents.skill_enhance_of_item(class));
    let started = skill_enhance::start(
        name,
        skill_enhance::StartFacts {
            feature_on: policy.is_enabled(Some("SkillEnhance"))?,
            skill,
            stone: stone
                .as_ref()
                .map(|(class, _, _)| (request.stone_item_id, class.as_str())),
            info,
            groups: Some(contents.skill_enhance_groups(name)),
        },
    );
    match started {
        Ok(session) => {
            state.session = Some(session);
            Ok(Vec::new())
        }
        Err(outcome) => {
            state.session = None;
            Ok(vec![system(SYSTEM, outcome)?])
        }
    }
}

/// SkillEnhanceSessionEndProcessor: closes any open session.
pub fn end(state: &mut State) {
    state.session = None;
}

/// SkillEnhanceSessionUseErgProcessor.
#[allow(clippy::too_many_arguments)]
pub async fn use_erg(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    state: &mut State,
    request: &wire::SkillEnhanceUseErg,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let Some(session) = state.session.as_mut() else {
        return Ok(vec![system(SYSTEM, Outcome::GeneralCannotFindSession)?]);
    };
    let journal =
        super::minimum_item_logs::named_journal(pool, log.flags, "SkillEnhance_Session").await;
    let mut tx = begin(pool, owner, assets, cid).await?;
    let erg = item(&mut tx, cid, request.erg_item_id).await?;
    let Some((class, count, _)) = erg.filter(|(_, n, _)| *n >= request.use_count) else {
        return Ok(vec![system(SYSTEM, Outcome::UseErgNotEnoughItem)?]);
    };
    // MakeMaterialProcessor reads Type of a null material: unknown classes throw.
    let material = contents
        .skill_enhance_material(&class)
        .ok_or_else(fail)?
        .clone();
    if let Err(outcome) = skill_enhance::can_use_material(session, &material, request.use_count) {
        return Ok(vec![system(SYSTEM, outcome)?]);
    }
    if request.use_count <= 0 || count < request.use_count {
        return Err(fail());
    }
    consume(&mut tx, cid, request.erg_item_id, request.use_count).await?;
    let inventory = slots_packet(
        &mut tx,
        owner,
        assets,
        cid,
        &[request.erg_item_id],
        Vec::new(),
    )
    .await?;
    let mut random = assets.skill_enhance_random.lock().await;
    let mut next = |min: i32, max: i32| super::minimum_enchant::next_range(&mut random, min, max);
    let mut updated = session.clone();
    skill_enhance::use_material(&mut updated, &material, request.use_count, &mut next)
        .ok_or_else(fail)?;
    tx.commit().await?;
    *session = updated;
    super::minimum_item_logs::write_requested_ledger(
        pool,
        cid,
        journal,
        LOG_DESTROY,
        ACTION_USE_ERG,
        &utf16(&class),
        request.use_count,
        log.flags,
    )
    .await;
    Ok(vec![
        inventory,
        packet(
            Kind::SkillEnhanceUseErgResult,
            Message::SkillEnhanceUseErgResult(wire::SkillEnhanceUseErgResult {
                result: true,
                current: session.success_ratio,
                max: session.max_success_ratio(),
            }),
        )?,
    ])
}

/// SkillEnhanceSessionProcessor: RollDice, then OnDiceSuccess/OnDiceFail; the session
/// closes either way.
#[allow(clippy::too_many_arguments)]
pub async fn roll(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    state: &mut State,
    additional: Option<&[i64]>,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let Some(session) = state.session.take() else {
        return Ok(vec![system(SYSTEM, Outcome::GeneralCannotFindSession)?]);
    };
    let journal =
        super::minimum_item_logs::named_journal(pool, log.flags, "SkillEnhance_Session").await;
    let mut tx = begin(pool, owner, assets, cid).await?;
    let dice = assets
        .skill_enhance_random
        .lock()
        .await
        .next(skill_enhance::MAX_PERCENTAGE)
        .map_err(|_| fail())?;
    // FindRuneItem: every ID must be an owned item (GetItem(id).ItemClass).
    let mut rune: Option<(i64, String)> = None;
    for id in additional.unwrap_or_default() {
        let (class, _, _) = item(&mut tx, cid, *id).await?.ok_or_else(fail)?;
        if rune.is_none() && skill_enhance::is_rune(&class) {
            rune = Some((*id, class));
        }
    }
    let (stone_class, _, stone_attributes) = item(&mut tx, cid, session.stone_item_id)
        .await?
        .ok_or_else(fail)?;
    // Item.HasSkillEnhanceProtected: VALUE "SKILLENHANCE_PROTECTED" with Arg > 0.
    let protected = stone_attributes
        .get("VALUE")
        .is_some_and(|a| a.value.to_uppercase() == "SKILLENHANCE_PROTECTED" && a.arg > 0);
    let decided = skill_enhance::roll(
        &session,
        dice,
        rune.as_ref().map(|(_, c)| c.as_str()),
        protected,
    );
    let action = if decided.success {
        ACTION_DICE_SUCCESS
    } else {
        ACTION_DICE_FAIL
    };
    let mut changed = Vec::new();
    let mut destroyed: Vec<(String, i32)> = Vec::new();
    let mut given = Vec::new();
    let mut ids = entity_ids.into_iter();
    if decided.consume_rune
        && let Some((id, class)) = &rune
    {
        consume(&mut tx, cid, *id, 1).await?;
        changed.push(*id);
        destroyed.push((class.clone(), 1));
    }
    let mut extra = Vec::new();
    if decided.give_protected_stone {
        let attributes = Attributes::new(vec![Attribute {
            name: "VALUE".to_owned(),
            value: "SKILLENHANCE_PROTECTED".to_owned(),
            arg: 1,
            arg2: 0,
        }])
        .map_err(|_| fail())?;
        let key = utf16(&stone_class);
        let applied = super::minimum_give::apply(
            &mut tx,
            owner,
            assets,
            cid,
            &[],
            &[Request {
                class: &key,
                count: 1,
                bound: false,
                equipment: None,
                attributes: &attributes,
            }],
            &mut || ids.next().ok_or_else(fail),
        )
        .await?
        // GiveItemFail_NoEmptySlot sends the stone by mail, which is not migrated.
        .map_err(|_| fail())?;
        extra.extend(applied.slots.iter().cloned());
        given.push(stone_class.clone());
    }
    if decided.consume_stone {
        consume(&mut tx, cid, session.stone_item_id, 1).await?;
        changed.push(session.stone_item_id);
        destroyed.push((stone_class.clone(), 1));
    }
    let inventory = slots_packet(&mut tx, owner, assets, cid, &changed, extra).await?;
    match &decided.enhance {
        Some(e) => {
            sqlx::query("INSERT INTO game_skill_enhances(skill_row_id,group_key,index_key,type,reduce_durability,max_durability_bonus) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(skill_row_id) DO UPDATE SET group_key=EXCLUDED.group_key,index_key=EXCLUDED.index_key,type=EXCLUDED.type,reduce_durability=EXCLUDED.reduce_durability,max_durability_bonus=EXCLUDED.max_durability_bonus")
                .bind(e.skill_row_id)
                .bind(e.group)
                .bind(e.key)
                .bind(i16::from(e.kind))
                .bind(e.reduce_durability)
                .bind(e.max_durability_bonus)
                .execute(&mut *tx)
                .await?;
        }
        None => {
            sqlx::query("DELETE FROM game_skill_enhances WHERE skill_row_id=$1")
                .bind(session.skill_row_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    if !state.updated.contains(&session.skill_row_id) {
        state.updated.push(session.skill_row_id);
    }
    let changed_packet = changed_message(&mut tx, cid, &state.updated).await?;
    let enhances = load(&mut tx, cid)
        .await?
        .into_iter()
        .filter(|(_, e)| e.skill_row_id == session.skill_row_id)
        .map(|(_, e)| Some(brief(&e)))
        .collect();
    tx.commit().await?;
    for (class, count) in &destroyed {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            LOG_DESTROY,
            action,
            &utf16(class),
            *count,
            log.flags,
        )
        .await;
    }
    for class in &given {
        super::minimum_item_logs::write_requested_ledger(
            pool,
            cid,
            journal,
            LOG_GIVE,
            action,
            &utf16(class),
            1,
            log.flags,
        )
        .await;
    }
    let result = packet(
        Kind::SkillEnhanceResult,
        Message::SkillEnhanceResult(wire::SkillEnhanceResult {
            skill_name: Some(session.skill.clone()),
            stone_item: Some(stone_class),
            success_ratio: session.success_ratio,
            additional_item_classes: Some(
                rune.filter(|_| !decided.success)
                    .map(|(_, c)| Some(c))
                    .into_iter()
                    .collect(),
            ),
            result: decided.success,
            additional_item_destroyed: decided.additional_destroyed,
            stone_protected: decided.stone_protected,
            enhances: Some(enhances),
        }),
    )?;
    // OnDiceSuccess flushes before the result; OnDiceFail sends the result first.
    Ok(if decided.success {
        vec![inventory, changed_packet, result]
    } else {
        vec![inventory, result, changed_packet]
    })
}

/// FlushChangedEnhances: every skill changed since the login (a removed enhance is a
/// Type 0 entry).
async fn changed_message(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    updated: &[i64],
) -> Result<OutgoingPacket, MinimumError> {
    let rows = load(tx, cid).await?;
    let mut out = Vec::new();
    for row in updated {
        let skill: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT skill_utf16 FROM game_skills WHERE row_id=$1 AND cid=$2")
                .bind(row)
                .bind(cid)
                .fetch_optional(&mut **tx)
                .await?;
        let Some(skill) = skill else {
            continue;
        };
        let value = rows
            .iter()
            .find(|(_, e)| e.skill_row_id == *row)
            .map(|(_, e)| brief(e))
            .unwrap_or(Brief {
                group_key: 0,
                index_key: 0,
                kind: skill_enhance::TYPE_NONE,
                reduce_durability: 0,
                max_durability_bonus: 0,
            });
        out.push((text(&skill)?, Some(value)));
    }
    packet(
        Kind::SkillEnhanceChanged,
        Message::SkillEnhanceChanged(StrDict(Some(out))),
    )
}
