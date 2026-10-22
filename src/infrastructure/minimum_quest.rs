//! QuestOwner of the selected first Fiona: QuestCollection rows, the login (first quest
//! and NotifyLevel auto-gives), saves and QuestProgressMessage (PushQuestProgressMessage).
use super::{character_lobby::UserKey, minimum_assets::Assets, minimum_character::MinimumError};
use crate::{
    domain::{
        character_stats::Arithmetic,
        map_partition::OutgoingPacket,
        quest::{Context, GoalAchievement, OwnedQuest, QuestClearTime, QuestContents, QuestOwner},
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message},
    },
};
use sqlx::{PgPool, Postgres, Transaction};

fn fail() -> MinimumError {
    MinimumError::Conflict
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn text(raw: &[u8]) -> Result<String, MinimumError> {
    if !raw.len().is_multiple_of(2) || raw.len() > 400 {
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

/// CLR profile and civil offset the quest messages depend on (TodayQuest hash,
/// AchieveGoalInfo DateTime.Now).
#[derive(Clone, Copy)]
pub struct QuestClock {
    pub arithmetic: Arithmetic,
    pub civil_offset_seconds: i64,
}

/// DateTime.UtcNow in SQL datetime units (1/300 s since 1900-01-01).
pub(super) fn now_units() -> Result<i64, MinimumError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| fail())?;
    let millis = i64::try_from(now.as_millis()).map_err(|_| fail())?;
    Ok((millis + 2_208_988_800_000) * 3 / 10)
}

type QuestRow = (i64, Vec<u8>, i32, i32, i32, i32, i64, i32, Option<i64>);
type GoalRow = (i32, i32, i16, i16, i64);
type ClearRow = (Vec<u8>, i32, i32, i32, i32, i32, i16);

/// QuestOwner.Load rows (QuestCollection created when missing); callers run InitQuestStatus.
pub(super) async fn load(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    contents: &QuestContents,
) -> Result<QuestOwner, MinimumError> {
    sqlx::query("INSERT INTO game_quest_collections(owner_id,permission) VALUES($1,3) ON CONFLICT(owner_id) DO NOTHING")
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    let quests: Vec<QuestRow> = sqlx::query_as(
        "SELECT row_id,quest_utf16,revealed,status,clear_count,play_count,last_play_units300,today_play_count,named_kill_units300 FROM game_quests WHERE owner_id=$1 ORDER BY row_id LIMIT 4097",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let goals: Vec<GoalRow> = sqlx::query_as(
        "SELECT goal_id,status,clear_count,regened_ap,last_clear_units300 FROM game_quest_goal_achievements WHERE owner_id=$1 ORDER BY row_id LIMIT 65537",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    let clears: Vec<ClearRow> = sqlx::query_as(
        "SELECT quest_utf16,difficulty,player_count,clear_time,play_rank,clear_info,is_old FROM game_quest_clear_times WHERE owner_id=$1 ORDER BY row_id LIMIT 65537",
    )
    .bind(cid)
    .fetch_all(&mut **tx)
    .await?;
    if quests.len() > 4096 || goals.len() > 65536 || clears.len() > 65536 {
        return Err(fail());
    }
    let quests = quests
        .into_iter()
        .map(
            |(row_id, quest, revealed, status, clear, play, last, today, named)| {
                Ok(OwnedQuest {
                    row_id: Some(row_id),
                    quest: text(&quest)?,
                    revealed,
                    status,
                    clear_count: clear,
                    play_count: play,
                    last_play: last,
                    today_play_count: today,
                    named_kill: named,
                })
            },
        )
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let goals = goals
        .into_iter()
        .map(|(goal, status, clear, ap, last)| {
            Ok(GoalAchievement {
                goal,
                status,
                clear_count: u8::try_from(clear).map_err(|_| fail())?,
                regened_ap: u8::try_from(ap).map_err(|_| fail())?,
                last_clear: last,
            })
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    let clears = clears
        .into_iter()
        .map(|(quest, difficulty, players, time, rank, info, old)| {
            Ok(QuestClearTime {
                quest: text(&quest)?,
                difficulty,
                player_count: players,
                clear_time: time,
                play_rank: rank,
                clear_info: info,
                is_old: old != 0,
            })
        })
        .collect::<Result<Vec<_>, MinimumError>>()?;
    QuestOwner::load(quests, goals, clears, contents).map_err(|_| fail())
}

/// Save of the changed Quest rows (new rows take the next entity IDs).
pub(super) async fn save(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
    original: &QuestOwner,
    owner: &QuestOwner,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<(), MinimumError> {
    for q in &owner.quests {
        match q.row_id {
            None => {
                sqlx::query("INSERT INTO game_quests(row_id,owner_id,quest_utf16,revealed,status,clear_count,play_count,last_play_units300,today_play_count,named_kill_units300) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
                    .bind(ids.next().ok_or_else(fail)?)
                    .bind(cid)
                    .bind(utf16(&q.quest))
                    .bind(q.revealed)
                    .bind(q.status)
                    .bind(q.clear_count)
                    .bind(q.play_count)
                    .bind(q.last_play)
                    .bind(q.today_play_count)
                    .bind(q.named_kill)
                    .execute(&mut **tx)
                    .await?;
            }
            Some(row_id) => {
                if original.quests.iter().any(|o| o == q) {
                    continue;
                }
                sqlx::query("UPDATE game_quests SET revealed=$3,status=$4,clear_count=$5,play_count=$6,last_play_units300=$7,today_play_count=$8,named_kill_units300=$9 WHERE row_id=$1 AND owner_id=$2")
                    .bind(row_id)
                    .bind(cid)
                    .bind(q.revealed)
                    .bind(q.status)
                    .bind(q.clear_count)
                    .bind(q.play_count)
                    .bind(q.last_play)
                    .bind(q.today_play_count)
                    .bind(q.named_kill)
                    .execute(&mut **tx)
                    .await?;
            }
        }
    }
    Ok(())
}

/// ItemOwner.GetItems("coupon_cash_raid_1").Count() (item rows, not counts).
pub(super) async fn raid_coupons(
    tx: &mut Transaction<'_, Postgres>,
    cid: i64,
) -> Result<i64, MinimumError> {
    Ok(sqlx::query_scalar(
        "SELECT count(*) FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2",
    )
    .bind(cid)
    .bind(utf16("coupon_cash_raid_1"))
    .fetch_one(&mut **tx)
    .await?)
}

/// QuestProgressMessage(GetQuestProgress(), GetAchievedGoals()) at `now`.
pub(super) fn progress_packet(
    owner: &QuestOwner,
    ctx: Context<'_>,
    clock: QuestClock,
    now: i64,
) -> Result<OutgoingPacket, MinimumError> {
    let progress = owner
        .progress(ctx, now, clock.arithmetic)
        .map_err(|_| fail())?
        .into_iter()
        .map(|p| {
            Some(wire::QuestProgressInfo {
                id: Some(p.id),
                progress: p.progress,
                clear_count: p.clear_count,
                today_quest: p.today_quest,
                appear_named_monster: p.appear_named_monster,
                available: p.available,
                revealed: p.revealed,
                max_play_count: p.max_play_count,
                play_count: p.play_count,
                clear_times: Some(
                    p.clear_times
                        .into_iter()
                        .map(|c| {
                            Some(wire::ClearTimeElement {
                                difficulty: c.difficulty,
                                member_count: c.player_count,
                                clear_time: c.clear_time,
                                play_rank: c.play_rank,
                                clear_info: c.clear_info,
                                is_old: c.is_old,
                            })
                        })
                        .collect(),
                ),
            })
        })
        .collect();
    // AchieveGoalInfo: (int)(DateTime.Now - LastClearTime).TotalSeconds.
    let local = now + clock.civil_offset_seconds * 300;
    let goals = owner
        .goals
        .iter()
        .map(|g| {
            let seconds = (local - g.last_clear) as f64 / 300.0;
            let diff = if seconds > -2_147_483_649.0 && seconds < 2_147_483_648.0 {
                seconds.trunc() as i32
            } else {
                i32::MIN
            };
            Some(wire::AchieveGoal {
                id: g.goal,
                status: g.status,
                clear_count: g.clear_count,
                regened_ap: g.regened_ap,
                last_clear_time_diff: diff,
            })
        })
        .collect();
    let message = Message::QuestProgress(wire::QuestProgress {
        progress: Some(progress),
        achieved_goals: Some(goals),
    });
    Ok(OutgoingPacket {
        category: Kind::QuestProgress.category(),
        body: wire::encode(Kind::QuestProgress, Some(&message), DEFAULT_MAX_BODY)
            .map_err(|_| fail())?
            .to_vec(),
    })
}

async fn level(tx: &mut Transaction<'_, Postgres>, cid: i64) -> Result<i32, MinimumError> {
    let (connected, level): (bool, i32) =
        sqlx::query_as("SELECT is_connected,level FROM game_character_info WHERE id=$1")
            .bind(cid)
            .fetch_one(&mut **tx)
            .await?;
    if !connected {
        return Err(fail());
    }
    Ok(level)
}

/// Entity IDs the quest login needs (one per quest it gives).
pub async fn login_openings(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
) -> Result<usize, MinimumError> {
    let contents = assets
        .npc
        .as_ref()
        .and_then(|n| n.quests.as_ref())
        .ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let level = level(&mut tx, cid).await?;
    let quests = load(&mut tx, cid, contents).await?;
    tx.commit().await?;
    Ok(quests.login_openings(contents, level))
}

/// PlayerEntity QuestOwner construction and SelectCharacter NotifyLevel: the first quest,
/// the auto-given quests up to the level, Save, then PushQuestProgressMessage.
pub async fn login(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    entity_ids: Vec<i64>,
    clock: QuestClock,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let contents = npc.quests.as_ref().ok_or_else(fail)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
        let level = level(&mut tx, cid).await?;
        let original = load(&mut tx, cid, contents).await?;
        if original.login_openings(contents, level) != entity_ids.len() {
            return Err(fail());
        }
        // Load (InitQuestStatus), the constructor's first quest and NotifyLevel, saved.
        let mut quests = original.clone();
        quests.init_status(contents);
        quests.login(contents, level);
        let mut ids = entity_ids.into_iter();
        save(&mut tx, cid, &original, &quests, &mut ids).await?;
        let coupons = raid_coupons(&mut tx, cid).await?;
        tx.commit().await?;
        // PushQuestProgressMessage needs CharacterLevel > 0.
        if level <= 0 {
            return Ok(Vec::new());
        }
        let ctx = Context {
            contents,
            policy: &npc.policy,
            cafe: false,
            raid_coupons: coupons,
        };
        Ok(vec![progress_packet(&quests, ctx, clock, now_units()?)?])
    })
    .await
    .map_err(|_| fail())?
}

/// QueryQuestProgressMessage: PushQuestProgressMessage of the current state.
pub async fn query_progress(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    clock: QuestClock,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let contents = npc.quests.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let level = level(&mut tx, cid).await?;
    let original = load(&mut tx, cid, contents).await?;
    let mut quests = original.clone();
    quests.init_status(contents);
    quests.level = level;
    save(&mut tx, cid, &original, &quests, &mut std::iter::empty()).await?;
    let coupons = raid_coupons(&mut tx, cid).await?;
    tx.commit().await?;
    if level <= 0 {
        return Ok(Vec::new());
    }
    let ctx = Context {
        contents,
        policy: &npc.policy,
        cafe: false,
        raid_coupons: coupons,
    };
    Ok(vec![progress_packet(&quests, ctx, clock, now_units()?)?])
}

/// QueryQuestDigestProcessor's checks before the digest: QuestOwner.CheckConstraint
/// (play count ignored in practice mode), then RequiredStoryLineID/RequiredPhase
/// against the character's story progress. `Ok(None)` passes; `Ok(Some(name))` is the
/// QuestConstraintResult the original answers with.
#[allow(clippy::too_many_arguments)]
pub async fn digest_constraint(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    quest: &str,
    difficulty: i32,
    practice: bool,
    story: Option<(&str, &str)>,
) -> Result<Option<&'static str>, MinimumError> {
    use crate::domain::quest::Constraint;
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let contents = npc.quests.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let level = level(&mut tx, cid).await?;
    let mut quests = load(&mut tx, cid, contents).await?;
    quests.init_status(contents);
    quests.level = level;
    let coupons = raid_coupons(&mut tx, cid).await?;
    let phase: Option<Option<Vec<u8>>> =
        match story {
            None => None,
            Some((line, _)) => sqlx::query_scalar(
                "SELECT phase_utf16 FROM game_story_progress WHERE cid=$1 AND story_line_utf16=$2",
            )
            .bind(cid)
            .bind(
                line.encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<u8>>(),
            )
            .fetch_optional(&mut *tx)
            .await?,
        };
    tx.commit().await?;
    let ctx = Context {
        contents,
        policy: &npc.policy,
        cafe: false,
        raid_coupons: coupons,
    };
    let result = quests
        .check_constraint(ctx, quest, difficulty, practice)
        .map_err(|_| fail())?;
    let name = match result {
        Constraint::Success => None,
        Constraint::InvalidQuest => Some("InvalidQuest"),
        Constraint::NoSuchDifficulty => Some("NoSuchDifficulty"),
        Constraint::LevelConstraintFail => Some("LevelConstraintFail"),
        Constraint::RequiredStatusFail => Some("RequiredStatusFail"),
        Constraint::MaxPlayCountFail => Some("MaxPlayCountFail"),
    };
    if name.is_some() {
        return Ok(name);
    }
    if let Some((_, phases)) = story {
        let phase = phase
            .flatten()
            .map(|p| {
                let (pairs, rest) = p.as_chunks::<2>();
                if !rest.is_empty() {
                    return Err(fail());
                }
                String::from_utf16(
                    &pairs
                        .iter()
                        .map(|c| u16::from_le_bytes(*c))
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| fail())
            })
            .transpose()?;
        // progress == null || !phaseList.Contains(progress.PhaseID).
        if !phase.is_some_and(|p| phases.split(';').any(|x| x == p)) {
            return Ok(Some("InvalidStoryPhase"));
        }
    }
    Ok(None)
}
