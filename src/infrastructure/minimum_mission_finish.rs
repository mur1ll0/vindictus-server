//! MicroPlay quest completion for the solo host: informer events through the
//! TargetTracker (Trigger_QuestGoal with AddQuestGoalReward and QuestTargetResult), the
//! success scene with the CheckQuestClearTime schedule, the per-minute job and time over,
//! the rewards (GiveReward_Season1Success/MainGoal/QuestFail/Null with RewardCharacter)
//! and the EndGame result (FinishGameMessage), then ReturnToTown.
use super::{
    character_lobby::UserKey,
    minimum_assets::Assets,
    minimum_character::MinimumError,
    minimum_growth::GrowthContext,
    minimum_mission::{Mission, inventory_packet, packet},
    minimum_quest::QuestClock,
};
use crate::domain::{
    feature_policy::FeaturePolicy,
    map_partition::OutgoingPacket,
    mission::MissionContents,
    mission_goal::{self as goal, Achieved, GoalReward, GoalTrigger, RewardFacts, RewardLists},
    npc_script::QuestFinished,
    quest::{Context, QuestOwner},
};
use crate::protocol::{
    inventory::Dict,
    mission::{
        FinishGame, Message, QuestSummary, QuestTargetResult, ReturnFromQuest, StrDict,
        SummaryGoal, SummaryReward,
    },
};
use sqlx::PgPool;
use std::time::{Duration, Instant};

fn fail() -> MinimumError {
    MinimumError::Conflict
}

/// MicroPlayResult.
pub mod result {
    pub const SUCCESS: u8 = 0;
    pub const SWEAR_VIOLATION: u8 = 1;
    pub const TIME_OVER: u8 = 2;
    pub const GIVE_UP_QUEST: u8 = 3;
}

/// MicroPlayer.PlayerState of the rewarding functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerState {
    InGame,
    Rewarding,
    Finished,
}

/// MicroPlayer rewarding functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewardKind {
    Season1Success,
    MainGoal,
    /// GiveReward_QuestFail(result, isPersonalFail).
    QuestFail(u8, bool),
    Null,
}

/// Scheduler jobs of the MicroPlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    EssentialCleared,
    MainGoalCleared(i32),
    Minute,
    TimeOver,
    Reward(RewardKind),
}

/// EndGame kept by the frontend proxy (PeerData.EndGameOp).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndGame {
    pub story_bsp: String,
    pub story_entity: String,
    pub leave_party_if_town: bool,
}

/// The MicroPlay and MicroPlayer state of the quest completion.
#[derive(Clone, Debug)]
pub struct Completion {
    pub cid: i64,
    pub tracker: Option<goal::TargetTracker>,
    pub achieved_main: Vec<GoalReward>,
    pub achieved_sub: Vec<GoalReward>,
    pub achieved_ids: Vec<i32>,
    /// AchievedGoalInfoDict when the player joined: goal → (Status, AchieveGoalInfo).
    pub achieved_info: Vec<(i32, i32, Achieved)>,
    pub today_quest: bool,
    pub today_play_count: i32,
    /// MicroPlayer.BossKilled: monster type → kills.
    pub boss_kills: Vec<(String, i32)>,
    pub xe_play_rank: i32,
    /// GivenExp, GivenGold, GivenAP.
    pub given: (i32, i32, i32),
    /// MicroPlay.State Finished.
    pub finished: bool,
    pub player_state: PlayerState,
    pub give_up: bool,
    pub first_start: Option<Instant>,
    /// StartTime: the instant and DateTime.UtcNow in SQL datetime units.
    pub start: Option<(Instant, i64)>,
    pub clear_time_violations: i32,
    pub jobs: Vec<(Instant, Job)>,
    /// UpdateQuestPlayCount requested by a boss kill and not yet applied.
    pub play_count_due: bool,
    pub play_count_updated: bool,
    pub end_game: Option<EndGame>,
    /// The frontend is in the StorySector state after ReturnFromQuest(ToStorySector).
    pub in_story_sector: bool,
}

impl Completion {
    pub fn new(cid: i64) -> Self {
        Self {
            cid,
            tracker: None,
            achieved_main: Vec::new(),
            achieved_sub: Vec::new(),
            achieved_ids: Vec::new(),
            achieved_info: Vec::new(),
            today_quest: false,
            today_play_count: 0,
            boss_kills: Vec::new(),
            xe_play_rank: 0,
            given: (0, 0, 0),
            finished: false,
            player_state: PlayerState::InGame,
            give_up: false,
            first_start: None,
            start: None,
            clear_time_violations: 0,
            jobs: Vec::new(),
            play_count_due: false,
            play_count_updated: false,
            end_game: None,
            in_story_sector: false,
        }
    }
    fn schedule(&mut self, at: Instant, job: Job) {
        self.jobs.push((at, job));
    }
}

/// Content and runtime values the completion reads.
#[derive(Clone, Copy)]
pub struct FinishContext<'a> {
    pub contents: &'a MissionContents,
    pub policy: &'a FeaturePolicy,
    pub clock: QuestClock,
}
impl FinishContext<'_> {
    fn integer(&self, key: &str) -> i32 {
        self.policy.get_integer(key).unwrap_or(0)
    }
    fn enabled(&self, key: &str) -> bool {
        self.policy.is_enabled(Some(key)).unwrap_or(false)
    }
    /// DateTime.Now in SQL datetime units.
    fn now_local(&self) -> Result<i64, MinimumError> {
        Ok(super::minimum_quest::now_units()? + self.clock.civil_offset_seconds * 300)
    }
}

/// AddPlayer's QueryQuestState and QueryPlayerInfo: TodayPlayCount, IsTodayQuest and the
/// AchievedGoalInfoList of the character.
pub(super) async fn load_player(
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    cid: i64,
    quest: &str,
    clock: QuestClock,
) -> Result<(Vec<(i32, i32, Achieved)>, i32, bool), MinimumError> {
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let contents = npc.quests.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, cid).await?;
    let mut quests = super::minimum_quest::load(&mut tx, cid, contents).await?;
    let coupons = super::minimum_quest::raid_coupons(&mut tx, cid).await?;
    tx.commit().await?;
    quests.init_status(contents);
    let ctx = Context {
        contents,
        policy: &npc.policy,
        cafe: false,
        raid_coupons: coupons,
    };
    let today = quests
        .is_today_quest(ctx, quest, super::minimum_quest::now_units()?, clock.arithmetic)
        .map_err(|_| fail())?;
    let today_play_count = quests.owned(quest).map_or(0, |q| q.today_play_count);
    let achieved = quests
        .goals
        .iter()
        .map(|g| {
            (
                g.goal,
                g.status,
                Achieved {
                    clear_count: g.clear_count,
                    regened_ap: g.regened_ap,
                    last_clear: g.last_clear,
                },
            )
        })
        .collect();
    Ok((achieved, today_play_count, today))
}

/// MoveSectorProcessor in the Wait state: InitializeTracker, StartTimer and the
/// per-minute job.
pub fn start(mission: &mut Mission, contents: &MissionContents, now: Instant) -> Result<(), MinimumError> {
    let c = &mut mission.completion;
    let tracker = contents
        .tracker(&mission.quest, mission.swear_id, &[c.cid])
        .map_err(|_| fail())?;
    if tracker.uncompiled > 0 {
        tracing::warn!(
            patterns = tracker.uncompiled,
            "quest goal targets the regex engine rejects never match; values omitted"
        );
    }
    c.tracker = Some(tracker);
    if c.first_start.is_none() {
        c.clear_time_violations = 0;
        c.first_start = Some(now);
        mission.min_clear_time_reduced = 0;
    }
    c.start = Some((now, super::minimum_quest::now_units()?));
    if !mission.digest.is_hunting_quest {
        c.schedule(now + Duration::from_secs(60), Job::Minute);
    }
    Ok(())
}

/// The earliest scheduled job.
pub fn next_job(mission: &Mission) -> Option<Instant> {
    mission.completion.jobs.iter().map(|(at, _)| *at).min()
}

/// MicroPlayInformer.InformToOtherService for the queued events: TargetTracker.Process
/// and the triggered goals, in event order.
pub fn drain(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    now: Instant,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    while !mission.events.is_empty() {
        let events = std::mem::take(&mut mission.events);
        for (actor, event) in events {
            let Some(tracker) = mission.completion.tracker.as_mut() else {
                continue;
            };
            for trigger in tracker.process(actor, &event) {
                trigger_quest_goal(mission, ctx, trigger, now, &mut packets)?;
            }
        }
    }
    Ok(packets)
}

/// MicroPlayer.AddQuestGoalReward (null with a reward penalty, give-up or no difficulty).
fn add_quest_goal_reward(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    goal_id: i32,
) -> Result<Option<GoalReward>, MinimumError> {
    let c = &mut mission.completion;
    if c.give_up {
        return Ok(None);
    }
    let (Some(info), Some((exp, gold, _))) = (
        ctx.contents.goal_info(goal_id),
        ctx.contents
            .difficulty_reward(&mission.quest, mission.option.difficulty),
    ) else {
        return Ok(None);
    };
    let relation = ctx
        .contents
        .quest_goals(&mission.quest)
        .map_err(|_| fail())?
        .iter()
        .find(|g| g.id == goal_id)
        .map_or(0, |g| g.relation);
    let achieved = c
        .achieved_info
        .iter()
        .find(|(g, _, _)| *g == goal_id)
        .map(|(_, _, a)| *a);
    let reward = goal::goal_reward(
        info,
        (exp, gold),
        (
            ctx.integer("ExpRate_Combat"),
            ctx.integer("GoldRate_Combat"),
            ctx.integer("APRate_Combat"),
        ),
        achieved,
        c.today_quest,
        ctx.now_local()?,
    );
    if relation == 1 || relation == 2 {
        c.achieved_sub.push(reward.clone());
    } else {
        c.achieved_main.push(reward.clone());
    }
    if !c.achieved_ids.contains(&goal_id) {
        c.achieved_ids.push(goal_id);
    }
    Ok(Some(reward))
}

fn target_result(
    name: &str,
    goal: i32,
    success: bool,
    reward: Option<&GoalReward>,
) -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::QuestTargetResult(QuestTargetResult {
        character_name: Some(name.to_owned()),
        goal_id: goal,
        is_goal_success: success,
        // The original constructor assigns IsQuestSuccess to itself.
        is_quest_success: false,
        exp: reward.map_or(0, |r| r.exp),
        gold: reward.map_or(0, |r| r.gold),
        ap: reward.map_or(0, |r| r.ap),
    }))
}

/// MicroPlay.Trigger_QuestGoal for the solo host.
fn trigger_quest_goal(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    t: GoalTrigger,
    now: Instant,
    packets: &mut Vec<OutgoingPacket>,
) -> Result<(), MinimumError> {
    let name = mission.member.character_id.clone().unwrap_or_default();
    if !t.success {
        let who = if t.cid == mission.completion.cid { name.as_str() } else { "" };
        packets.push(target_result(who, t.goal, false, None)?);
        if matches!(t.relation, 0 | 2 | 3) && mission.in_game && !mission.completion.finished {
            quest_fail(mission, result::SWEAR_VIOLATION, now);
        }
        return Ok(());
    }
    if t.relation != 3 {
        if t.is_party {
            if mission.completion.give_up {
                packets.push(target_result("", t.goal, true, None)?);
            } else {
                match add_quest_goal_reward(mission, ctx, t.goal)? {
                    Some(r) => packets.push(target_result("", t.goal, true, Some(&r))?),
                    None => tracing::warn!("quest goal reward not found; values omitted"),
                }
            }
        } else if t.cid == mission.completion.cid
            && let Some(r) = add_quest_goal_reward(mission, ctx, t.goal)?
        {
            packets.push(target_result(&name, t.goal, true, Some(&r))?);
        }
    }
    if mission.completion.finished {
        return Ok(());
    }
    let single = mission
        .completion
        .tracker
        .as_ref()
        .is_some_and(|tr| tr.is_single_boss);
    let job = if t.relation == 0 && t.main_goal_achieved || t.relation == 3 && single {
        Job::EssentialCleared
    } else if t.relation == 3 {
        Job::MainGoalCleared(t.goal)
    } else {
        return Ok(());
    };
    packets.push(packet(&Message::QuestSuccessSceneStart)?);
    let delay = check_quest_clear_time(mission, ctx, now, packets)?;
    mission
        .completion
        .schedule(now + Duration::from_secs(delay), job);
    Ok(())
}

/// MicroPlay.CheckQuestClearTime: the success scene delay, held back to the quest's
/// minimum clear time with the "too fast" warning (no cheat permission).
fn check_quest_clear_time(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    now: Instant,
    packets: &mut Vec<OutgoingPacket>,
) -> Result<u64, MinimumError> {
    let mut finish = ctx.integer("MicroPlayFinishTime") / 1000;
    if finish == 0 {
        finish = 50;
    }
    let min = mission.digest.min_clear_time;
    let mut wait = if mission.digest.is_season2 && min != 0 {
        let bosses = i32::try_from(mission.boss_killed.len()).map_err(|_| fail())?;
        bosses
            .wrapping_add(1)
            .wrapping_mul(min)
            .wrapping_mul(60)
            .wrapping_sub(finish)
    } else {
        min.wrapping_mul(60).wrapping_sub(finish)
    };
    wait = wait.wrapping_sub(mission.min_clear_time_reduced);
    let c = &mut mission.completion;
    if wait > 0 && ctx.enabled("CheckMinClearTime") {
        let elapsed = c
            .first_start
            .map_or(0, |s| now.saturating_duration_since(s).as_secs());
        let elapsed = i32::try_from(elapsed).unwrap_or(i32::MAX);
        if wait > elapsed {
            let remaining = wait.wrapping_add(finish).wrapping_sub(elapsed);
            if remaining > 10 {
                c.clear_time_violations += 1;
                packets.push(super::minimum_shop::system_message(
                    0,
                    "GameUI_Heroes_Too_Fast_Monster_Kill",
                    &[remaining.to_string()],
                )?);
                return Ok(u64::try_from(remaining).unwrap_or(0));
            }
            // Every player without the cheat permission keeps the remaining delay, but no
            // warning was sent: the original then returns the normal finish time.
            packets.push(super::minimum_shop::system_message(
                0,
                "[Cheat] Ignore Time violation by cheat permission",
                &[],
            )?);
        }
    }
    Ok(u64::try_from(finish).unwrap_or(0))
}

/// MicroPlay.GiveReward_QuestFail(failReason): the play finishes and the player in game
/// is rewarded as failed (or with nothing when the play was not in game).
fn quest_fail(mission: &mut Mission, reason: u8, now: Instant) {
    let in_game = mission.in_game && !mission.completion.finished;
    let c = &mut mission.completion;
    c.finished = true;
    if c.player_state == PlayerState::InGame {
        let kind = if in_game {
            RewardKind::QuestFail(reason, false)
        } else {
            RewardKind::Null
        };
        c.schedule(now, Job::Reward(kind));
    }
}

/// Removes and returns the jobs due at `now`, earliest first.
fn take_due(mission: &mut Mission, now: Instant) -> Option<Job> {
    let jobs = &mut mission.completion.jobs;
    let index = jobs
        .iter()
        .enumerate()
        .filter(|(_, (at, _))| *at <= now)
        .min_by_key(|(_, (at, _))| *at)
        .map(|(i, _)| i)?;
    Some(jobs.remove(index).1)
}

/// Runtime values of the rewarding jobs.
pub struct RewardContext<'a> {
    pub pool: &'a PgPool,
    pub owner: &'a UserKey<'a>,
    pub assets: &'a Assets,
    pub growth: GrowthContext,
}

/// Runs the jobs due at `now` (the scheduler of the MicroPlay service thread).
pub async fn run_jobs(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    rc: &RewardContext<'_>,
    ids: &mut (dyn Iterator<Item = i64> + Send),
    now: Instant,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    while let Some(job) = take_due(mission, now) {
        match job {
            Job::Minute => {
                if mission.digest.is_hunting_quest {
                    continue;
                }
                mission.events.push((0, "minute_elapse".to_owned()));
                packets.extend(drain(mission, ctx, now)?);
                if mission.in_game && !mission.completion.finished {
                    let elapsed = mission
                        .completion
                        .start
                        .map_or(0.0, |(s, _)| now.saturating_duration_since(s).as_secs_f64() / 60.0);
                    let limit = mission.digest.time_limit;
                    if limit > 0 && elapsed > f64::from(limit) {
                        mission
                            .completion
                            .schedule(now + Duration::from_secs(5), Job::TimeOver);
                    } else {
                        mission
                            .completion
                            .schedule(now + Duration::from_secs(60), Job::Minute);
                    }
                }
            }
            Job::TimeOver => quest_fail(mission, result::TIME_OVER, now),
            Job::EssentialCleared => {
                let c = &mut mission.completion;
                if c.clear_time_violations > 0 {
                    c.clear_time_violations -= 1;
                }
                if !mission.in_game || c.finished {
                    continue;
                }
                c.finished = true;
                let level = mission.member.level;
                let triggers = match mission.completion.tracker.as_mut() {
                    Some(tracker) => {
                        let mut t = tracker.process(-1, &format!("member_level_{level:02}"));
                        t.extend(tracker.on_quest_finished());
                        t
                    }
                    None => Vec::new(),
                };
                for t in triggers {
                    trigger_quest_goal(mission, ctx, t, now, &mut packets)?;
                }
                mission
                    .completion
                    .schedule(now, Job::Reward(RewardKind::Season1Success));
            }
            Job::MainGoalCleared(goal_id) => {
                let c = &mut mission.completion;
                if c.clear_time_violations > 0 {
                    c.clear_time_violations -= 1;
                }
                if !mission.in_game || c.finished {
                    continue;
                }
                if mission.completion.give_up {
                    packets.push(target_result("", goal_id, true, None)?);
                } else {
                    match add_quest_goal_reward(mission, ctx, goal_id)? {
                        Some(r) => packets.push(target_result("", goal_id, true, Some(&r))?),
                        None => tracing::warn!("quest goal reward not found; values omitted"),
                    }
                }
                mission
                    .completion
                    .schedule(now, Job::Reward(RewardKind::MainGoal));
            }
            Job::Reward(kind) => {
                packets.extend(reward(mission, ctx, rc, ids, kind, now).await?);
                packets.extend(drain(mission, ctx, now)?);
            }
        }
    }
    Ok(packets)
}

/// RewardCharacter values of one rewarding function.
struct RewardOp {
    success: bool,
    clear_time_sec: i32,
    exp: i32,
    gold: i32,
    ap: i32,
    quest_goals: Vec<(i32, i64)>,
    items: Vec<(String, i32)>,
}

/// RewardCharacterProcessor results: given EXP, gold, AP, items and the story sector.
struct RewardResult {
    exp: i32,
    gold: i32,
    ap: i32,
    items: Vec<(String, i32)>,
    story_sector: i32,
    packets: Vec<OutgoingPacket>,
}

fn reward_facts(mission: &Mission, ctx: FinishContext<'_>) -> RewardFacts {
    let difficulty = mission.option.difficulty;
    let c = &mission.completion;
    let christmas = ctx.enabled("ChristmasCounterEvent").then(|| {
        (
            ctx.integer("ChristmasCounterEvent_EXP_Rate"),
            ctx.integer("ChristmasCounterEvent_Gold_Rate"),
            ctx.integer("ChristmasCounterEvent_AP_Rate"),
        )
    });
    RewardFacts {
        quest_id: mission.quest.clone(),
        level: mission.member.level,
        max_level: ctx.integer("MaxLevel"),
        quest_level: mission.digest.quest_level,
        is_season2: mission.digest.is_season2,
        difficulty,
        game_mode: ctx
            .contents
            .difficulty_info(&mission.quest, difficulty)
            .map_or(0, |d| d.0),
        difficulty_max_level: ctx
            .contents
            .difficulty_reward(&mission.quest, difficulty)
            .map_or(0, |d| d.2),
        today_quest: c.today_quest,
        today_play_count: c.today_play_count,
        free_quest: mission.digest.is_free_quest,
        free_repeat_penalty: ctx.enabled("FreeRepeatPenalty"),
        // CharacterSummary.DailyFreePlayCount (AddMicroPlayCount) is not migrated.
        daily_free_play_count: 0,
        main_goal_achieved: c.tracker.as_ref().is_some_and(|t| t.main_goal_achieved),
        extreme_mode: ctx.enabled("ExtremeMode"),
        xe_play_rank: c.xe_play_rank,
        combat_gold_renewal: ctx.enabled("CombatGold_Renewal"),
        christmas,
        target_event: (
            ctx.integer("TargetQuestEvent_Exp"),
            ctx.integer("TargetQuestEvent_Gold"),
            ctx.policy.get_string("TargetQuestEvent_Quest").to_owned(),
            ctx.policy.get_string("TargetQuestEvent_Text").to_owned(),
        ),
    }
}

fn summary_lines(lines: &[goal::SummaryLine]) -> Option<Vec<Option<SummaryReward>>> {
    Some(
        lines
            .iter()
            .map(|l| {
                Some(SummaryReward {
                    desc: Some(l.desc.clone()),
                    arg1: l.arg1,
                    arg2: l.arg2,
                    value: l.value,
                })
            })
            .collect(),
    )
}

/// A MicroPlayer rewarding function: the reserved boss items, the reward calculation,
/// RewardCharacter and, except for the main-goal reward, the EndGame result.
async fn reward(
    mission: &mut Mission,
    ctx: FinishContext<'_>,
    rc: &RewardContext<'_>,
    ids: &mut (dyn Iterator<Item = i64> + Send),
    kind: RewardKind,
    now: Instant,
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let c = &mission.completion;
    if c.player_state == PlayerState::Rewarding {
        mission
            .completion
            .schedule(now + Duration::from_secs(5), Job::Reward(kind));
        return Ok(Vec::new());
    }
    if c.player_state == PlayerState::Finished && c.give_up || c.player_state != PlayerState::InGame {
        return Ok(Vec::new());
    }
    let mut packets = Vec::new();
    if kind == RewardKind::Null {
        // GiveReward_Null → Leave → SendNullEndGameOp: the proxy leaves the party.
        mission.completion.player_state = PlayerState::Finished;
        packets.push(packet(&Message::Kicked)?);
        return Ok(packets);
    }
    mission.completion.player_state = PlayerState::Rewarding;
    let practice = mission.option.is_practice_mode;
    let (success, result) = match kind {
        RewardKind::QuestFail(reason, _) => (false, reason),
        _ => (true, result::SUCCESS),
    };
    if matches!(kind, RewardKind::Season1Success) {
        packets.extend(give_reserved_items(mission, rc, ids).await?);
    }
    if let RewardKind::QuestFail(..) = kind {
        let c = &mut mission.completion;
        c.achieved_main.clear();
        c.achieved_sub.clear();
        c.achieved_ids.clear();
    }
    let facts = reward_facts(mission, ctx);
    let mut lists = RewardLists::new(&facts);
    if !mission.completion.give_up {
        lists.add_bonuses(&facts);
    }
    let game_mode = facts.game_mode;
    let c = &mut mission.completion;
    let calculation = goal::calculate(
        facts.main_goal_achieved,
        &mut c.achieved_main,
        &mut c.achieved_sub,
        game_mode,
        mission.monster_kill_exp,
        &lists,
        &|f| ctx.enabled(f),
    )
    .map_err(|_| fail())?;
    let mut summary = QuestSummary {
        quest_id: Some(mission.quest.clone()),
        swear_id: mission.swear_id,
        is_today_quest: false,
        result,
        last_progress: 0,
        current_progress: 0,
        cleared_goals: Dict(Some(
            calculation
                .summary
                .cleared_goals
                .iter()
                .map(|&(g, (exp, gold, ap))| {
                    (
                        g,
                        Some(SummaryGoal {
                            goal_id: g,
                            exp,
                            gold,
                            ap,
                        }),
                    )
                })
                .collect(),
        )),
        battle_exp: summary_lines(&calculation.summary.battle_exp),
        etc_exp: summary_lines(&calculation.summary.etc_exp),
        main_gold: summary_lines(&calculation.summary.main_gold),
        etc_gold: summary_lines(&calculation.summary.etc_gold),
        quest_ap: summary_lines(&calculation.summary.quest_ap),
        etc_ap: summary_lines(&calculation.summary.etc_ap),
        exp: calculation.exp,
        gold: calculation.gold,
        ap: calculation.ap,
        reward_item: StrDict(Some(Vec::new())),
    };
    let mut story = (String::new(), String::new());
    if practice {
        // Reward_SendRewardOperation in practice mode: no RewardCharacter.
        let line = |desc: &str| {
            Some(vec![Some(SummaryReward {
                desc: Some(desc.to_owned()),
                arg1: 0,
                arg2: 0,
                value: 0,
            })])
        };
        summary.cleared_goals = Dict(Some(Vec::new()));
        summary.battle_exp = Some(Vec::new());
        summary.etc_exp = line("PracticeMode");
        summary.main_gold = Some(Vec::new());
        summary.etc_gold = line("PracticeMode");
        summary.quest_ap = Some(Vec::new());
        summary.etc_ap = line("PracticeMode");
        summary.reward_item = StrDict(Some(Vec::new()));
        summary.exp = 0;
        summary.gold = 0;
        summary.ap = 0;
    } else {
        let c = &mission.completion;
        let elapsed = c
            .start
            .map_or(0, |(s, _)| now.saturating_duration_since(s).as_secs());
        let op = RewardOp {
            success,
            clear_time_sec: i32::try_from(elapsed).unwrap_or(i32::MAX),
            exp: calculation.exp.wrapping_sub(c.given.0),
            gold: calculation.gold.wrapping_sub(c.given.1),
            ap: calculation.ap.wrapping_sub(c.given.2),
            quest_goals: calculation.quest_goals.clone(),
            items: calculation.items.clone(),
        };
        if op.exp < 0 || op.gold < 0 || op.ap < 0 {
            tracing::error!("reward op has a negative value; values omitted");
        }
        for (g, _) in &op.quest_goals {
            mission.events.push((mission.completion.cid, format!("ClearGoal {g}")));
        }
        // CharacterRank (UpdateRankBasis) is not migrated.
        let given = reward_character(mission, ctx, rc, ids, &op).await;
        match given {
            Ok(r) => {
                let c = &mut mission.completion;
                c.given.0 = c.given.0.wrapping_add(r.exp);
                c.given.1 = c.given.1.wrapping_add(r.gold);
                c.given.2 = c.given.2.wrapping_add(r.ap);
                packets.extend(r.packets);
                summary.reward_item = StrDict(Some(r.items));
                if r.story_sector != -1
                    && let Some((bsp, entity)) = ctx.contents.story_sector(r.story_sector)
                {
                    story = (bsp.to_owned(), entity.to_owned());
                }
            }
            Err(_) => tracing::error!("RewardCharacter failed; values omitted"),
        }
        let (current, last) = progress(mission, ctx)?;
        summary.current_progress = current;
        summary.last_progress = last;
    }
    let c = &mut mission.completion;
    match kind {
        RewardKind::MainGoal => {
            c.player_state = PlayerState::InGame;
            return Ok(packets);
        }
        RewardKind::QuestFail(..) => {
            story = (String::new(), String::new());
        }
        _ => {}
    }
    // EndGameType: Continueable (or StoryEnd for a story quest) on success, SoloEnd on a
    // solo failure. Restarting from the ship (ReturnToShip) is not migrated, so the result
    // screen never offers it.
    let story_end = mission.digest.required_story_line.is_some();
    let continueable = success && !story_end;
    packets.push(packet(&Message::FinishGame(FinishGame {
        summary: Some(summary),
        has_story_sector: !story.0.is_empty(),
        can_restart: false,
        can_continue_finished_quest: continueable,
    }))?);
    c.end_game = Some(EndGame {
        story_bsp: story.0,
        story_entity: story.1,
        leave_party_if_town: true,
    });
    c.player_state = PlayerState::Finished;
    if let RewardKind::QuestFail(..) = kind {
        c.give_up = true;
    }
    if success {
        let cid = c.cid;
        let quest = mission.quest.clone();
        mission.events.push((cid, format!("ClearCount {quest}")));
        let difficulty = if ctx
            .contents
            .difficulty_info(&quest, mission.option.difficulty)
            .is_some_and(|d| d.0 == 3)
        {
            "Hero".to_owned()
        } else {
            mission.option.difficulty.to_string()
        };
        mission.events.push((cid, format!("ClearDiffi {quest} {difficulty}")));
        if mission.digest.party_limit > 4 {
            mission.events.push((cid, format!("ClearRaid {quest}")));
        }
        if mission.completion.today_quest {
            mission.events.push((cid, format!("ClearTodayQuest {quest}")));
        }
    }
    Ok(packets)
}

/// CalculateCurrentQuestProgress and CalculateLastQuestProgress.
fn progress(mission: &Mission, ctx: FinishContext<'_>) -> Result<(i32, i32), MinimumError> {
    let goals: Vec<(i32, i32)> = ctx
        .contents
        .quest_goals(&mission.quest)
        .map_err(|_| fail())?
        .iter()
        .map(|g| (g.id, g.info.weight))
        .collect();
    let c = &mission.completion;
    let stored: Vec<(i32, i32)> = c.achieved_info.iter().map(|(g, s, _)| (*g, *s)).collect();
    let (season2, difficulty) = (mission.digest.is_season2, mission.option.difficulty);
    Ok((
        goal::quest_progress(&goals, &stored, Some(&c.achieved_ids), season2, difficulty),
        goal::quest_progress(&goals, &stored, None, season2, difficulty),
    ))
}

/// SectorSequence.GiveReservedItems: EvilCore.LootAll of the boss cores for the player
/// (refused with a message under a minimum clear time violation).
async fn give_reserved_items(
    mission: &mut Mission,
    rc: &RewardContext<'_>,
    ids: &mut (dyn Iterator<Item = i64> + Send),
) -> Result<Vec<OutgoingPacket>, MinimumError> {
    let mut packets = Vec::new();
    let Some(sequence) = mission.sequence.as_mut() else {
        return Ok(packets);
    };
    let mut items: Vec<(String, i32)> = Vec::new();
    let violation = mission.completion.clear_time_violations > 0;
    for core in sequence.cores.iter_mut().filter(|c| c.is_boss) {
        if violation {
            packets.push(super::minimum_shop::system_message(
                0,
                "GameUI_Heroes_MinClearTimeViolation_Cannot_Loot",
                &[],
            )?);
            continue;
        }
        for (kind, list) in std::mem::take(&mut core.items) {
            items.extend(list.into_iter().map(|class| (class, kind)));
        }
    }
    let cid = mission.completion.cid;
    for (class, kind) in items {
        let mut next = || ids.next().ok_or_else(fail);
        match super::minimum_mission::give_reward(
            rc.pool, rc.owner, rc.assets, mission, cid, &class, 1, kind, &mut next,
        )
        .await
        {
            Ok(more) => packets.extend(more),
            Err(_) => tracing::warn!("reserved boss item refused by the give planner; values omitted"),
        }
    }
    Ok(packets)
}

/// RewardCharacterProcessor in one transaction: GiveExp/GiveAP with their pushes, gold
/// and item rewards, UpdateQuest/UpdateQuestStatus with Save, and the story NotifyEvent.
async fn reward_character(
    mission: &Mission,
    ctx: FinishContext<'_>,
    rc: &RewardContext<'_>,
    ids: &mut (dyn Iterator<Item = i64> + Send),
    op: &RewardOp,
) -> Result<RewardResult, MinimumError> {
    let cid = mission.completion.cid;
    let npc = rc.assets.npc.as_ref().ok_or_else(fail)?;
    let quests = npc.quests.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(rc.pool, rc.owner, cid).await?;
    let mut packets = super::minimum_growth::give_reward(
        &mut tx,
        rc.owner,
        rc.assets,
        cid,
        i64::from(op.exp),
        op.ap,
        rc.growth,
        ids,
    )
    .await?;
    let mut given_gold = op.gold;
    let mut given_items: Vec<(String, i32)> = Vec::new();
    let mut next = || ids.next().ok_or_else(fail);
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(u16::to_le_bytes).collect() };
    if op.gold > 0 {
        let class = utf16("gold");
        let gold = super::minimum_give::Request {
            class: &class,
            count: op.gold,
            bound: false,
            equipment: None,
        };
        match super::minimum_give::apply(&mut tx, rc.owner, rc.assets, cid, &[], &[gold], &mut next).await? {
            Ok(applied) => packets.push(inventory_packet(applied.slots)?),
            Err(_) => given_gold = 0,
        }
    }
    let rate = match ctx.policy.get_string("Event_RewardItemRate") {
        "" => 1,
        _ => ctx.integer("Event_RewardItemRate"),
    };
    for (key, count) in &op.items {
        let count = count.wrapping_mul(rate);
        if key.split('>').count() == 2 {
            // DoChangeItem (CraftItem of one class into another) is not migrated.
            tracing::warn!("quest item change reward is not migrated; values omitted");
            continue;
        }
        if crate::domain::mission::item_class(key) != key || count <= 0 {
            continue;
        }
        let class = utf16(key);
        let request = super::minimum_give::Request {
            class: &class,
            count,
            bound: false,
            equipment: None,
        };
        match super::minimum_give::apply(&mut tx, rc.owner, rc.assets, cid, &[], &[request], &mut next).await? {
            Ok(applied) => {
                packets.push(inventory_packet(applied.slots)?);
                given_items.push((key.clone(), count));
            }
            Err(_) => tracing::warn!("quest item reward refused by the give planner; values omitted"),
        }
    }
    // QuestOwner.UpdateQuest and UpdateQuestStatus, then Save (or Undo).
    let original = super::minimum_quest::load(&mut tx, cid, quests).await?;
    let mut owner: QuestOwner = original.clone();
    owner.init_status(quests);
    let now = ctx.now_local()?;
    let add_clear = |g: i32, cleared: i64, count: &mut u8, regen: &mut u8, last: &mut i64| {
        ctx.contents
            .goal_info(g)
            .map(|i| i.add_clear_count(cleared, count, regen, last))
            .is_some()
    };
    let updated = owner.update_quest(
        quests,
        &mission.quest,
        &op.quest_goals,
        (
            mission.option.difficulty,
            1,
            op.clear_time_sec,
            mission.completion.xe_play_rank,
            1,
        ),
        now,
        &add_clear,
    ) && owner.update_quest_status(quests, &mission.quest);
    if updated {
        super::minimum_quest::save(&mut tx, cid, &original, &owner, ids).await?;
    } else {
        tracing::warn!("cannot update quest after the reward; values omitted");
    }
    let (story_sector, story) = super::minimum_story::quest_finished(
        &mut tx,
        rc.assets,
        cid,
        &QuestFinished {
            quest: &mission.quest,
            success: op.success,
            boss_killed: &mission.completion.boss_kills,
            is_host: true,
        },
    )
    .await?;
    packets.extend(story);
    tx.commit().await?;
    Ok(RewardResult {
        exp: op.exp,
        gold: given_gold,
        ap: op.ap,
        items: given_items,
        story_sector,
        packets,
    })
}

/// UpdateQuestPlayCount after the first boss kill (StartTime, not ignoring today's count).
pub async fn update_play_count(
    mission: &mut Mission,
    pool: &PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
) -> Result<(), MinimumError> {
    let c = &mut mission.completion;
    if !c.play_count_due || c.play_count_updated {
        return Ok(());
    }
    c.play_count_due = false;
    c.play_count_updated = true;
    let Some((_, start)) = c.start else {
        return Ok(());
    };
    let npc = assets.npc.as_ref().ok_or_else(fail)?;
    let contents = npc.quests.as_ref().ok_or_else(fail)?;
    let mut tx = super::fiona_character_state::owned_first_fiona(pool, owner, c.cid).await?;
    let original = super::minimum_quest::load(&mut tx, c.cid, contents).await?;
    let coupons = super::minimum_quest::raid_coupons(&mut tx, c.cid).await?;
    let mut quests = original.clone();
    quests.init_status(contents);
    let ctx = Context {
        contents,
        policy: &npc.policy,
        cafe: false,
        raid_coupons: coupons,
    };
    match quests
        .update_play_count(ctx, &mission.quest, start, false)
        .map_err(|_| fail())?
    {
        None => return Ok(()),
        // Destroying the raid coupon is not migrated; the count is kept.
        Some(true) => tracing::warn!("raid coupon play is not migrated; values omitted"),
        Some(false) => {}
    }
    super::minimum_quest::save(&mut tx, c.cid, &original, &quests, &mut std::iter::empty()).await?;
    tx.commit().await?;
    Ok(())
}

/// Entity IDs reserved for one round of rewarding jobs (item rows, quest rows).
pub const REWARD_ENTITY_IDS: usize = 64;

/// InformMicroPlayEventProcessor for the solo host: false without a sector sequence
/// (FailMessage); outside a sector the event is dropped. "xe_play_rank" sets
/// XEPlayRank; weak points, fishing, gourd/pumpkin and golden ball drops and sector
/// events are not migrated. The event goes to the informer with the slot's player.
pub fn micro_play_event(mission: &mut Mission, slot: i32, text: &str) -> bool {
    let Some(sequence) = mission.sequence.as_ref() else {
        return false;
    };
    if sequence.current.is_none() {
        return true;
    }
    let own = slot == mission.member.slot;
    let actor = if own { mission.completion.cid } else { -1 };
    let split: Vec<&str> = text.split(' ').collect();
    match split[0] {
        "xe_play_rank" if split.len() >= 2 => match split[1].parse::<i32>() {
            // GetPlayer(slot) is null for another slot: the processor throws.
            Ok(rank) if own => mission.completion.xe_play_rank = rank,
            _ => return false,
        },
        "weakpoint_break" | "weakpoint_break_bonus" if split.len() >= 5 => {
            tracing::warn!("mission weak point event is not migrated; values omitted")
        }
        "snatch_fish" | "goldenball" | "sector_event" if split.len() >= 2 => {
            tracing::warn!("mission client event effect is not migrated; values omitted")
        }
        "kill" if split.len() >= 2
            && matches!(split[1], "gourd_for_player" | "pumpkin_for_player") =>
        {
            tracing::warn!("mission event core drop is not migrated; values omitted")
        }
        _ => {}
    }
    mission.events.push((actor, text.to_owned()));
    true
}

/// What ReturnToTownMessage does with the mission.
pub enum Return {
    /// No EndGame yet: FinishMicroPlay is not requested.
    Ignored,
    /// The proxy moved to the StorySector state; the mission stays for its return.
    StorySector(Vec<OutgoingPacket>),
    /// The party was left and the client returns to town.
    Town(Vec<OutgoingPacket>),
}

fn to_town() -> Result<OutgoingPacket, MinimumError> {
    packet(&Message::ReturnFromQuest(ReturnFromQuest {
        order: 0,
        story_sector_bsp: Some(String::new()),
        story_sector_entity: Some(String::new()),
    }))
}

/// ReturnToTownMessage: with an EndGame, FinishMicroPlay(ReturnToTown) and the proxy's
/// ReturnFromQuest(ToTown) — LeaveParty (KickedMessage) then ReturnFromQuestMessage, or
/// ReturnFromQuestMessage(ToStorySector) then LeaveParty. In the StorySector state:
/// ReturnFromQuestMessage(ToTown) then LeaveParty.
pub fn return_to_town(mission: &mut Mission) -> Result<Return, MinimumError> {
    let c = &mut mission.completion;
    if c.in_story_sector {
        return Ok(Return::Town(vec![to_town()?, packet(&Message::Kicked)?]));
    }
    let Some(end) = c.end_game.clone() else {
        return Ok(Return::Ignored);
    };
    if !end.story_bsp.is_empty() {
        c.in_story_sector = true;
        return Ok(Return::StorySector(vec![
            packet(&Message::ReturnFromQuest(ReturnFromQuest {
                order: 2,
                story_sector_bsp: Some(end.story_bsp),
                story_sector_entity: Some(end.story_entity),
            }))?,
            packet(&Message::Kicked)?,
        ]));
    }
    Ok(Return::Town(vec![packet(&Message::Kicked)?, to_town()?]))
}
