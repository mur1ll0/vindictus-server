//! ManufactureManager state of the selected first Fiona (game_manufacture and
//! game_manufacture_recipes in load order), ManufactureInfoMessage (PushManufactureInfo,
//! also requested by ItemServiceProxy.Open) and the story changes AcquireManufacture,
//! LevelupManufacture and AcquireManufactureRecipe.
use super::minimum_character::MinimumError;
use crate::{
    domain::{
        manufacture::{Skill, State},
        map_partition::OutgoingPacket,
        npc_talk::ManufactureChange,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, ManufactureInfo, ManufactureLevelUp, Message},
        mission::StrDict,
    },
};
use sqlx::{Postgres, Transaction};

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

/// GetManufactureStatus and GetManufactureRecipes in row order.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<State, MinimumError> {
    let skills: Vec<(Vec<u8>, i32, i32)> = sqlx::query_as(
        "SELECT manufacture_utf16,grade,experience_point FROM game_manufacture WHERE cid=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let recipes: Vec<Vec<u8>> = sqlx::query_scalar(
        "SELECT recipe_utf16 FROM game_manufacture_recipes WHERE cid=$1 ORDER BY row_id LIMIT 65537",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if skills.len() > 4096 || recipes.len() > 65536 {
        return Err(fail());
    }
    let mut state = State::default();
    for (id, grade, experience) in skills {
        let id = text(&id)?;
        // ManufactureManager: EXP.Add throws on a repeated ManufacturelID.
        if state.skill(&id).is_some() {
            return Err(fail());
        }
        state.skills.push(Skill {
            id,
            grade,
            experience,
        });
    }
    state.recipes = recipes.iter().map(|r| text(r)).collect::<Result<_, _>>()?;
    Ok(state)
}

/// ManufactureInfoMessage(EXPDic, GradeDic, ManufactureRecipes).
pub(super) fn info_packet(state: &State) -> Result<OutgoingPacket, MinimumError> {
    let dict = |f: fn(&Skill) -> i32| {
        StrDict(Some(
            state.skills.iter().map(|s| (s.id.clone(), f(s))).collect(),
        ))
    };
    packet(
        Kind::ManufactureInfo,
        Message::ManufactureInfo(ManufactureInfo {
            exp: dict(|s| s.experience),
            grade: dict(|s| s.grade),
            recipes: Some(state.recipes.iter().cloned().map(Some).collect()),
        }),
    )
}

/// ItemServiceProxy.Open → QueryManufactureInfo.
pub(super) async fn query(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<OutgoingPacket, MinimumError> {
    info_packet(&load(tx, cid).await?)
}

/// Persists a story change decided by `npc_talk` and returns its frontend messages:
/// ManufactureLevelUpMessage (acquire and level-up) then ManufactureInfoMessage.
pub(super) async fn apply(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    change: &ManufactureChange,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    match change {
        ManufactureChange::Acquired(mid) => {
            sqlx::query(
                "INSERT INTO game_manufacture(cid,manufacture_utf16,grade,experience_point) VALUES($1,$2,0,0)",
            )
            .bind(cid)
            .bind(utf16(mid))
            .execute(&mut **tx)
            .await?;
            packets.push(level_up_packet(mid, 0)?);
        }
        ManufactureChange::LevelUp {
            id,
            grade,
            experience,
        } => {
            let updated = sqlx::query(
                "UPDATE game_manufacture SET grade=$3,experience_point=$4 WHERE cid=$1 AND manufacture_utf16=$2",
            )
            .bind(cid)
            .bind(utf16(id))
            .bind(grade)
            .bind(experience)
            .execute(&mut **tx)
            .await?;
            if updated.rows_affected() != 1 {
                return Err(fail());
            }
            packets.push(level_up_packet(id, *grade)?);
        }
        ManufactureChange::Recipe { id, new } => {
            if *new {
                // ManufactureRecipe.AcquireTime = DateTime.UtcNow.
                let ticks = super::lab_clock::utc_now().map_err(|_| fail())?.ticks();
                sqlx::query(
                    "INSERT INTO game_manufacture_recipes(cid,recipe_utf16,acquired_ticks) VALUES($1,$2,$3)",
                )
                .bind(cid)
                .bind(utf16(id))
                .bind(ticks)
                .execute(&mut **tx)
                .await?;
            }
        }
    }
    packets.push(query(tx, cid).await?);
    Ok(packets)
}

fn level_up_packet(mid: &str, grade: i32) -> Result<OutgoingPacket, MinimumError> {
    packet(
        Kind::ManufactureLevelUp,
        Message::ManufactureLevelUp(ManufactureLevelUp {
            mid: Some(mid.to_owned()),
            grade,
        }),
    )
}

/// ActionType.Manufacture / EventType.ManufactureCraftItem / ItemLogType ledgers.
const ACTION_MANUFACTURE: i16 = 12;
const EVENT_MANUFACTURE_CRAFT_ITEM: i16 = 51;
const LOG_GIVE: i16 = 2;
const LOG_DESTROY: i16 = 3;
/// SystemMessageCategory.System.
const SYSTEM: u8 = 0;

struct CraftRandom(crate::domain::item_colors::ColorRandom);
impl crate::domain::enchant::Random for CraftRandom {
    fn next_double(&mut self) -> f64 {
        self.0.next_double()
    }
    fn next(&mut self, maximum: i32) -> Option<i32> {
        self.0.next(maximum).ok()
    }
}

type RequestRow = (
    Vec<u8>,
    i32,
    bool,
    crate::domain::item_attribute::Attributes,
);

fn requests(rows: &[RequestRow]) -> Vec<super::minimum_give::Request<'_>> {
    rows.iter()
        .map(
            |(class, count, bound, attributes)| super::minimum_give::Request {
                class,
                count: *count,
                bound: *bound,
                equipment: None,
                attributes,
            },
        )
        .collect()
}

/// ManufactureCraftMessage → ManufactureCraftItem(RecipeID, 1, PartsIDList) →
/// ManufactureManager.ProcessProduct without Combine Craft parts: CraftItem(materials,
/// targets with QUALITY on equipment), the experience step, Save and
/// PushManufactureInfoMessage. Refusals answer SendOperationFailedDialog.
pub async fn craft(
    pool: &sqlx::PgPool,
    owner: &super::character_lobby::UserKey<'_>,
    assets: &super::minimum_assets::Assets,
    cid: i64,
    request: &wire::ManufactureCraft,
    entity_ids: Vec<i64>,
    log: super::minimum_item_logs::Context,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    use crate::domain::{
        item_attribute::{self, Attribute, Attributes},
        manufacture::{self, CraftRefusal},
    };
    let contents = assets.enhance.as_ref().ok_or_else(fail)?;
    let policy = contents.policy().ok_or_else(fail)?;
    let manufacture_contents = contents.manufacture();
    // A null RecipeID throws in Dictionary.TryGetValue.
    let recipe_id = request.recipe_id.as_deref().ok_or_else(fail)?;
    let dialog = |name: &str| -> Result<Vec<OutgoingPacket>, MinimumError> {
        Ok(vec![super::minimum_equip::item_fail(name)?])
    };
    let operation = super::minimum_item_logs::operation_journal(pool, log.flags).await;
    let journal =
        super::minimum_item_logs::named_journal(pool, log.flags, "ManufactureCraftItem").await;
    let mut ids = entity_ids.into_iter();
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let (connected, name): (bool, Option<Vec<u8>>) =
        sqlx::query_as("SELECT is_connected,name_utf16 FROM game_character_info WHERE id=$1")
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
    let mut state = load(&mut tx, cid).await?;
    let feature_on = match manufacture_contents
        .recipe(recipe_id)
        .and_then(|r| r.feature.as_deref())
    {
        Some(f) => policy.is_enabled(Some(f))?,
        None => true,
    };
    let plan =
        match manufacture::plan_craft(manufacture_contents, &state, recipe_id, 1, |_| feature_on)
            .map_err(|_| fail())?
        {
            Ok(plan) => plan,
            Err(CraftRefusal::NoRecipe) => return dialog("CraftItemFail_NoRecipe"),
        };
    // A single target with PartsIDList goes to ItemCombinationSession when the target has
    // CombineCraftInfo (B3 item 5): not migrated.
    if plan.targets.len() == 1
        && request
            .parts_id_list
            .as_ref()
            .is_some_and(|p| !p.is_empty())
    {
        return Err(fail());
    }
    let skill = state
        .skill(&plan.recipe.manufacture_id)
        .ok_or_else(fail)?
        .clone();
    let mut random = CraftRandom(crate::domain::item_colors::ColorRandom::from_seed(
        assets.craft_random_seed(),
    ));
    // GiveItemRequest.FromDictionaryWithBindCheck: ItemClassEx, a known class and
    // IsCharacterBinded = FeatureMatrix.IsEnable(ItemClassInfo.Bind).
    let crafter = text(name.as_deref().unwrap_or_default())?;
    let mut gives: Vec<RequestRow> = Vec::new();
    for (class_ex, count) in &plan.targets {
        let Ok((class, mut attributes)) = item_attribute::parse_class_ex(class_ex) else {
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
        let bound = policy.is_enabled(bind.as_deref())?;
        if indexed.equippable {
            let quality = match contents.quality_type(&key) {
                Some(kind) => manufacture::roll_quality(
                    contents.quality_dice(kind),
                    skill.experience,
                    plan.recipe.experience_required,
                    &mut random,
                )
                .map_err(|_| fail())?,
                None => 2,
            };
            // Attribute.Add throws on an ItemClassEx that already carries QUALITY.
            if attributes.get(super::minimum_item_stat::QUALITY).is_some() {
                return Err(fail());
            }
            let mut list = attributes.as_slice().to_vec();
            list.push(Attribute {
                name: super::minimum_item_stat::QUALITY.to_owned(),
                value: crafter.clone(),
                arg: quality,
                arg2: 0,
            });
            attributes = Attributes::new(list).map_err(|_| fail())?;
        }
        gives.push((key, *count, bound, attributes));
    }
    let destroys: Vec<RequestRow> = plan
        .materials
        .iter()
        .map(|(c, n)| (utf16(c), *n, false, Attributes::default()))
        .collect();
    let mut next_id = || ids.next().ok_or_else(fail);
    let applied = match super::minimum_give::apply(
        &mut tx,
        owner,
        assets,
        cid,
        &requests(&destroys),
        &requests(&gives),
        &mut next_id,
    )
    .await?
    {
        Ok(a) => a,
        Err(crate::domain::give_item::GiveError::NotEnoughItem) => {
            return dialog("CraftItemFail_NotEnoughMaterial");
        }
        // Leftovers of the source go to the mailbox, which is not migrated.
        Err(_) => return Err(fail()),
    };
    let max = manufacture_contents.max_exp(&skill.id, skill.grade);
    let event_exp = policy.get_integer("manufacture_event_exp")?;
    let (gained, experience) = manufacture::craft_experience(
        max,
        skill.experience,
        plan.recipe.experience_required,
        plan.recipe.base_reward_exp,
        event_exp,
        &mut random,
    )
    .map_err(|_| fail())?;
    if experience != skill.experience {
        sqlx::query(
            "UPDATE game_manufacture SET experience_point=$3 WHERE cid=$1 AND manufacture_utf16=$2",
        )
        .bind(cid)
        .bind(utf16(&skill.id))
        .bind(experience)
        .execute(&mut *tx)
        .await?;
        if let Some(s) = state.skills.iter_mut().find(|s| s.id == skill.id) {
            s.experience = experience;
        }
    }
    tx.commit().await?;
    super::minimum_item_logs::write_give(
        pool,
        cid,
        operation,
        EVENT_MANUFACTURE_CRAFT_ITEM,
        &applied,
        log,
    )
    .await;
    for (rows, kind) in [(&plan.materials, LOG_DESTROY), (&plan.targets, LOG_GIVE)] {
        for (class, count) in rows {
            super::minimum_item_logs::write_requested_ledger(
                pool,
                cid,
                journal,
                kind,
                ACTION_MANUFACTURE,
                &utf16(class),
                *count,
                log.flags,
            )
            .await;
        }
    }
    let mut packets = Vec::new();
    if let Some(value) = gained {
        packets.push(super::minimum_shop::system_message(
            SYSTEM,
            "GameUI_Heroes_Manufacture_GetExperience",
            &[value.to_string(), experience.to_string(), max.to_string()],
        )?);
    }
    packets.push(packet(
        Kind::UpdateInventoryInfo,
        Message::UpdateInventoryInfo(Some(applied.slots)),
    )?);
    packets.push(info_packet(&state)?);
    Ok(packets)
}
