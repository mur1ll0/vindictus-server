//! Character progression from the admin panel (B4.3c): EXP (the game's GiveExp with the
//! level-up chain), a target level (the EXP that reaches it), AP (GiveAP, capped like the
//! game), a title (Titles.AcquireTitle) and status effects (StatusEffectManager add/remove,
//! for example VIP), and "reach a level" (the level plus the mission quests of
//! `domain::progression_plan`, completed by the game's own QuestOwner success path). Same
//! transaction as the game's own changes, for an offline initialized laboratory Fiona; the
//! panel's audit row commits with the change.
use super::{
    admin_store::{self, AuditRecord},
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_growth::GrowthContext,
};
use sqlx::{PgPool, Postgres, Transaction};

#[derive(Clone, Copy, Debug)]
pub enum PanelGrowth<'a> {
    Exp(i64),
    /// Reach this level from the current one (the EXP the level table requires).
    Level(i32),
    /// Reach this level (when above the current one) and complete the plan's quests.
    Reach(i32),
    Ap(i32),
    Title(i32),
    /// Add (`Some(level, seconds)`, seconds None = the effect's own duration) or remove
    /// (`None`) a status effect by Type.
    Effect(&'a str, Option<(i32, Option<i32>)>),
}

#[derive(Debug)]
pub enum PanelGrowthOutcome {
    Applied {
        before: serde_json::Value,
        after: serde_json::Value,
    },
    /// The game's rules made no change (max level, unknown title or effect, effect that
    /// cannot overwrite the stored one, level not above the current one).
    Unchanged(&'static str),
    Online,
    Repeated,
}

fn fail() -> MinimumError {
    MinimumError::Conflict
}

async fn snapshot(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<serde_json::Value, sqlx::Error> {
    let (level, exp, ap, titles, quests, effects): (i32, i64, i32, i64, i64, Vec<String>) =
        sqlx::query_as(
            "SELECT c.level, c.exp, c.ap,
                (SELECT count(*) FROM game_titles t WHERE t.cid = c.id AND t.acquired),
                (SELECT count(*) FROM game_quests q WHERE q.owner_id = c.id AND q.clear_count > 0),
                ARRAY(SELECT encode(e.type_utf16, 'hex') || ':' || e.level
                      FROM game_status_effects e WHERE e.cid = c.id ORDER BY e.row_id)
         FROM game_character_info c WHERE c.id = $1",
        )
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    // Effect types are stored as UTF-16LE; show them as text.
    let effects: Vec<String> = effects
        .iter()
        .map(|e| {
            let (hex, level) = e.split_once(':').unwrap_or((e, ""));
            let bytes: Vec<u8> = (0..hex.len() / 2)
                .filter_map(|i| u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok())
                .collect();
            let units: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| u16::from_le_bytes(*p))
                .collect();
            format!("{}:{level}", String::from_utf16_lossy(&units))
        })
        .collect();
    Ok(serde_json::json!({
        "level": level, "exp": exp, "ap": ap, "titles": titles, "quests": quests, "effects": effects
    }))
}

/// EXP that takes a character at `level` with `exp` (progress inside the level) to `target`
/// with 1 EXP of progress, by the level table; None when out of the table or over i32.
fn exp_to(assets: &Assets, level: i32, exp: i64, target: i32) -> Option<i64> {
    let progression = assets.npc.as_ref()?.progression.as_ref()?;
    if target <= level || target > progression.max_level() {
        return None;
    }
    let mut total: i64 = 0;
    for l in level..target {
        total += i64::from(progression.level_exp(l)?);
    }
    let needed = total - exp + 1;
    (1..=i64::from(i32::MAX))
        .contains(&needed)
        .then_some(needed)
}

/// The content's facts of every quest for the plan.
fn quest_facts(assets: &Assets) -> Option<Vec<crate::domain::progression_plan::QuestFacts>> {
    let npc = assets.npc.as_ref()?;
    let contents = npc.quests.as_ref()?;
    let mission = npc.mission.as_ref()?;
    contents
        .quests()
        .iter()
        .map(|q| {
            let level = mission.quest_level(&q.id);
            Some(crate::domain::progression_plan::QuestFacts {
                id: q.id.clone(),
                level: level.map(|l| l.0),
                quest_set: q.quest_set,
                hunting: level.is_some_and(|l| l.1),
                enabled: npc.policy.is_enabled(q.feature.as_deref()).ok()?,
                difficulties: contents.difficulties_of(&q.id).to_vec(),
                requires: contents
                    .constraints_of(&q.id)
                    .iter()
                    .map(|(r, _)| r.clone())
                    .collect(),
            })
        })
        .collect()
}

/// The quests "reach a level" would complete; None without the quest contents or above
/// the level table.
pub fn reach_plan(assets: &Assets, level: i32) -> Option<crate::domain::progression_plan::Plan> {
    let max = assets.npc.as_ref()?.progression.as_ref()?.max_level();
    if !(1..=max).contains(&level) {
        return None;
    }
    Some(crate::domain::progression_plan::plan(
        level,
        &quest_facts(assets)?,
    ))
}

#[allow(clippy::too_many_arguments)]
pub async fn panel_change(
    pool: &PgPool,
    assets: &Assets,
    cid: i64,
    change: PanelGrowth<'_>,
    ctx: GrowthContext,
    service_id: i32,
    mut audit: AuditRecord<'_>,
) -> Result<PanelGrowthOutcome, MinimumError> {
    let user: Option<(i32, String, i64)> = sqlx::query_as(
        "SELECT u.id,u.name,u.local_revision FROM game_character_info c JOIN game_users u ON u.id=c.uid WHERE c.id=$1",
    )
    .bind(cid)
    .fetch_optional(pool)
    .await?;
    let (uid, name, revision) = user.ok_or_else(fail)?;
    let owner = UserKey {
        uid,
        name: &name,
        revision,
    };
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, &owner, cid).await?;
    let connected: bool =
        sqlx::query_scalar("SELECT is_connected FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut *tx)
            .await?;
    if connected {
        return Ok(PanelGrowthOutcome::Online);
    }
    let before = snapshot(&mut tx, cid).await?;
    // Item IDs for level-up rewards, minted like the panel's item changes.
    let mut authority = crate::application::character_creation::EntityAuthority::new(service_id);
    let mut ids = std::iter::from_fn(move || {
        let now = super::lab_clock::utc_now().ok()?;
        authority.allocate(now).ok().map(|a| a.entity_time.ticks())
    });
    let changed = match change {
        PanelGrowth::Exp(exp) => {
            if !(1..=i64::from(i32::MAX)).contains(&exp) {
                return Err(fail());
            }
            super::minimum_growth::give_reward(&mut tx, &owner, assets, cid, exp, 0, ctx, &mut ids)
                .await?;
            None
        }
        PanelGrowth::Level(target) => {
            let level = before["level"].as_i64().unwrap_or(0) as i32;
            let exp = before["exp"].as_i64().unwrap_or(0);
            match exp_to(assets, level, exp, target) {
                Some(exp) => {
                    super::minimum_growth::give_reward(
                        &mut tx, &owner, assets, cid, exp, 0, ctx, &mut ids,
                    )
                    .await?;
                    None
                }
                None => Some("nivel"),
            }
        }
        PanelGrowth::Reach(target) => {
            let level = before["level"].as_i64().unwrap_or(0) as i32;
            let exp = before["exp"].as_i64().unwrap_or(0);
            let plan = reach_plan(assets, target).ok_or_else(fail)?;
            if target < level {
                return Ok(PanelGrowthOutcome::Unchanged("nivel"));
            }
            if target > level {
                let exp = exp_to(assets, level, exp, target).ok_or_else(fail)?;
                super::minimum_growth::give_reward(
                    &mut tx, &owner, assets, cid, exp, 0, ctx, &mut ids,
                )
                .await?;
            }
            let contents = assets
                .npc
                .as_ref()
                .and_then(|n| n.quests.as_ref())
                .ok_or_else(fail)?;
            let original = super::minimum_quest::load(&mut tx, cid, contents).await?;
            let mut quests = original.clone();
            quests.init_status(contents);
            // LastPlayTime/LastClearTime in SQL datetime units (1/300 s since 1900).
            let now: i64 = sqlx::query_scalar(
                "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::BIGINT",
            )
            .fetch_one(&mut *tx)
            .await?;
            for q in &plan.quests {
                quests.complete_for_panel(contents, &q.id, q.difficulty, now);
            }
            super::minimum_quest::save(&mut tx, cid, &original, &quests, &mut ids).await?;
            None
        }
        PanelGrowth::Ap(ap) => {
            if !(1..=i32::MAX).contains(&ap) {
                return Err(fail());
            }
            super::minimum_growth::give_reward(&mut tx, &owner, assets, cid, 0, ap, ctx, &mut ids)
                .await?;
            None
        }
        PanelGrowth::Title(title) => {
            super::minimum_growth::acquire_title(&mut tx, &owner, assets, cid, title, ctx).await?;
            None
        }
        PanelGrowth::Effect(kind, add) => {
            let contents = &assets
                .cash_shop
                .as_ref()
                .ok_or_else(fail)?
                .customize()
                .status_effects;
            let done = match add {
                Some((level, seconds)) => {
                    super::minimum_status_effect::add(&mut tx, contents, cid, kind, level, seconds)
                        .await?
                }
                None => super::minimum_status_effect::remove(&mut tx, contents, cid, kind).await?,
            };
            (!done).then_some("efeito")
        }
    };
    let after = snapshot(&mut tx, cid).await?;
    if let Some(reason) = changed.or((before == after).then_some("sem_mudanca")) {
        return Ok(PanelGrowthOutcome::Unchanged(reason));
    }
    audit.before = Some(before.to_string());
    audit.after = Some(after.to_string());
    match admin_store::insert_audit(&mut tx, &audit).await {
        Ok(()) => {}
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            return Ok(PanelGrowthOutcome::Repeated);
        }
        Err(e) => return Err(e.into()),
    }
    tx.commit().await?;
    Ok(PanelGrowthOutcome::Applied { before, after })
}
