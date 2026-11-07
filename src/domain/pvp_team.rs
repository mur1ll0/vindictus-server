//! DeathMatchGame and GodMatchGame of the PvpService with PvpPlayTeam (B5 PV3b): two teams
//! (TeamRed, TeamBlue) of TeamSize players, the game starting 10 s after every player is
//! ready plus WaitBeforeStart, rounds until RoundToWin (DeathMatch) or one round to a god's
//! death (GodMatch), the MinClearTime delay, the rewards (signets, guild GP, rank basis)
//! without the penalized players, and the end confirmation. The effects and timers are the
//! Arena's (`pvp_arena`).
use super::pvp_arena::{BLUE, Effect, INVALID, Member, RED, Timer};
use std::collections::{BTreeMap, BTreeSet};

/// PvpCommandType values the team games send.
mod cmd {
    pub const START_GAME: i32 = 0;
    pub const END_GAME: i32 = 2;
    pub const ALL_PLAYER_READY: i32 = 3;
    pub const START_ROUND: i32 = 4;
    pub const END_ROUND: i32 = 5;
}
mod report {
    pub const HOSTED: i32 = 0;
    pub const KILL: i32 = 4;
    pub const REVIVE: i32 = 6;
    pub const GOD_KILL: i32 = 7;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    DeathMatch,
    GodMatch,
}

/// A Scheduler job of a team game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamTimer {
    /// OnAllPlayerReady(WaitBeforeStart), 10 s after the last ready player.
    AllReady,
    StartGame,
    StartRound,
    EndRound,
    EndGame,
    /// The 32 s end confirmation window: everyone still in the play leaves.
    ForceRemove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TeamState {
    WaitingRoom,
    InGame,
    GameEnded,
}

/// The DeathMatchConfig/GodMatchConfig and FeatureMatrix values of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub rule: Rule,
    pub game_length: i32,
    pub wait_before_start: i32,
    pub wait_between_round: i32,
    pub round_to_win: i32,
    pub win_signet: i32,
    pub draw_signet: i32,
    pub lose_signet: i32,
    pub min_clear_time: i32,
    pub is_guild: bool,
    pub has_event_reward: bool,
    /// FeatureMatrix CharacterRank.
    pub character_rank: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Stat {
    kill: i32,
    death: i32,
    dead: bool,
}

pub struct TeamGame {
    rules: Rules,
    state: TeamState,
    changed_ms: i64,
    round_start_ms: i64,
    game_start_ms: i64,
    red: Vec<usize>,
    blue: Vec<usize>,
    score: [i32; 2],
    count: [i32; 2],
    god_alive: [bool; 2],
    stats: BTreeMap<usize, Stat>,
    penalty: BTreeSet<usize>,
    members: BTreeMap<usize, Member>,
    /// Reward() clears Score and Stat: later reports change nothing.
    rewarded: bool,
}

fn side(team: i32) -> Option<usize> {
    match team {
        RED => Some(0),
        BLUE => Some(1),
        _ => None,
    }
}

impl TeamGame {
    pub fn new(rules: Rules, now_ms: i64) -> Self {
        Self {
            rules,
            state: TeamState::WaitingRoom,
            changed_ms: now_ms,
            round_start_ms: now_ms,
            game_start_ms: now_ms,
            red: Vec::new(),
            blue: Vec::new(),
            score: [0; 2],
            count: [0; 2],
            god_alive: [true; 2],
            stats: BTreeMap::new(),
            penalty: BTreeSet::new(),
            members: BTreeMap::new(),
            rewarded: false,
        }
    }
    pub fn state(&self) -> TeamState {
        self.state
    }
    /// PvpPlayTeam.AddPlayer of a slot to a team.
    pub fn add_player(&mut self, slot: usize, team: i32, member: Member) {
        match team {
            RED => self.red.push(slot),
            BLUE => self.blue.push(slot),
            _ => return,
        }
        self.members.insert(slot, member);
        self.stats.insert(slot, Stat::default());
    }
    /// PvpPlayTeam.GetPlayerTeamID.
    pub fn team_of(&self, slot: usize) -> i32 {
        if self.red.contains(&slot) {
            RED
        } else if self.blue.contains(&slot) {
            BLUE
        } else {
            INVALID
        }
    }
    fn team_slots(&self, team: i32) -> &[usize] {
        match team {
            RED => &self.red,
            BLUE => &self.blue,
            _ => &[],
        }
    }
    fn team_count(&self, team: i32) -> usize {
        self.team_slots(team).len()
    }
    fn without_penalty(&self, team: i32) -> Vec<usize> {
        self.team_slots(team)
            .iter()
            .copied()
            .filter(|s| !self.penalty.contains(s))
            .collect()
    }
    fn change_state(&mut self, state: TeamState, now_ms: i64) {
        self.state = state;
        self.changed_ms = now_ms;
    }

    /// GameInfo: TeamInfo {-1: "1,{remaining}" in game, else "0, 0"}; DeathMatch slots
    /// "kill,death,alive" in game ("0,0,0,0" before), GodMatch "".
    pub fn game_info(&self, now_ms: i64) -> super::pvp_arena::GameInfo {
        let in_game = self.state == TeamState::InGame;
        let team = if in_game {
            let passed = ((now_ms - self.changed_ms) / 1000) as i32;
            format!("1,{}", self.rules.game_length - passed)
        } else {
            "0, 0".to_owned()
        };
        let players = self
            .members
            .keys()
            .map(|s| {
                let text = match (self.rules.rule, in_game) {
                    (Rule::GodMatch, _) => String::new(),
                    (Rule::DeathMatch, true) => {
                        let st = self.stats.get(s).copied().unwrap_or_default();
                        format!("{},{},{}", st.kill, st.death, if st.dead { 0 } else { 1 })
                    }
                    (Rule::DeathMatch, false) => "0,0,0,0".to_owned(),
                };
                (*s as i32, text)
            })
            .collect();
        (vec![(-1, team)], players)
    }

    /// OnPlayerReady: with every player ready, OnAllPlayerReady 10 s later.
    pub fn on_player_ready(&mut self, all_ready: bool) -> Vec<Effect> {
        if all_ready {
            vec![Effect::Schedule {
                delay_ms: 10_000,
                timer: Timer::Team(TeamTimer::AllReady),
            }]
        } else {
            Vec::new()
        }
    }

    /// GivePanalty: the player gets no reward.
    pub fn penalty(&mut self, slot: usize) {
        if self.members.contains_key(&slot) {
            self.penalty.insert(slot);
        }
    }

    pub fn timer(&mut self, timer: TeamTimer, now_ms: i64) -> Vec<Effect> {
        let mut out = Vec::new();
        match timer {
            TeamTimer::AllReady => {
                let delay = self.rules.wait_before_start.max(0);
                out.push(Effect::Command(cmd::ALL_PLAYER_READY, delay.to_string()));
                out.push(Effect::Schedule {
                    delay_ms: (delay as u64) * 1000,
                    timer: Timer::Team(TeamTimer::StartGame),
                });
            }
            TeamTimer::StartGame => {
                if self.state == TeamState::WaitingRoom {
                    self.score = [0; 2];
                    self.start_round(true, now_ms, &mut out);
                }
            }
            TeamTimer::StartRound => self.start_round(false, now_ms, &mut out),
            TeamTimer::EndRound => self.end_round(now_ms, &mut out),
            TeamTimer::EndGame => self.end_game(now_ms, &mut out),
            TeamTimer::ForceRemove => {}
        }
        out
    }

    fn start_round(&mut self, first: bool, now_ms: i64, out: &mut Vec<Effect>) {
        self.count = [self.team_count(RED) as i32, self.team_count(BLUE) as i32];
        if self.rules.rule == Rule::GodMatch {
            self.god_alive = [true; 2];
            if self.count[0] == 0 || self.count[1] == 0 {
                self.end_game(now_ms, out);
                return;
            }
        } else {
            for st in self.stats.values_mut() {
                st.dead = false;
            }
        }
        let length = self.rules.game_length.to_string();
        if first {
            out.push(Effect::Command(cmd::START_GAME, length));
            self.game_start_ms = now_ms;
        } else {
            out.push(Effect::Command(cmd::START_ROUND, length));
        }
        self.change_state(TeamState::InGame, now_ms);
        self.round_start_ms = now_ms;
        out.push(Effect::Schedule {
            delay_ms: (self.rules.game_length.max(0) as u64) * 1000 + 200,
            timer: Timer::Team(TeamTimer::EndRound),
        });
    }

    fn end_round(&mut self, now_ms: i64, out: &mut Vec<Effect>) {
        if self.state != TeamState::InGame || self.rewarded {
            return;
        }
        let timed_out = now_ms - self.round_start_ms > i64::from(self.rules.game_length) * 1000;
        match self.rules.rule {
            Rule::DeathMatch => {
                if self.count[0] == 0 {
                    self.score[1] += 1;
                } else if self.count[1] == 0 {
                    self.score[0] += 1;
                } else {
                    if !timed_out {
                        return;
                    }
                    if self.count[0] > self.count[1] {
                        self.score[0] += 1;
                    } else if self.count[0] < self.count[1] {
                        self.score[1] += 1;
                    } else {
                        self.score[0] += 1;
                        self.score[1] += 1;
                    }
                }
            }
            Rule::GodMatch => {
                if !self.god_alive[0] || self.count[0] <= 0 {
                    self.score[1] += 1;
                } else if !self.god_alive[1] || self.count[1] <= 0 {
                    self.score[0] += 1;
                } else {
                    if !timed_out {
                        return;
                    }
                    self.score[0] += 1;
                    self.score[1] += 1;
                }
                self.god_alive = [true; 2];
            }
        }
        let won =
            self.score[0] >= self.rules.round_to_win || self.score[1] >= self.rules.round_to_win;
        if !won {
            if self.rules.rule == Rule::DeathMatch {
                self.change_state(TeamState::WaitingRoom, now_ms);
                out.push(Effect::Command(
                    cmd::END_ROUND,
                    self.rules.wait_between_round.to_string(),
                ));
                out.push(Effect::Schedule {
                    delay_ms: (self.rules.wait_between_round.max(0) as u64) * 1000,
                    timer: Timer::Team(TeamTimer::StartRound),
                });
            }
            return;
        }
        // MinClearTime: DeathMatch from the game's start, GodMatch from the state change.
        let since = match self.rules.rule {
            Rule::DeathMatch => self.game_start_ms,
            Rule::GodMatch => self.changed_ms,
        };
        let left = self.rules.min_clear_time - ((now_ms - since) / 1000) as i32;
        if left > 0 {
            out.push(Effect::Telepathy {
                format: "GameUI_Heroes_Too_Fast_Finish_Pvp",
                args: vec![left.to_string()],
            });
            out.push(Effect::Schedule {
                delay_ms: (left as u64) * 1000,
                timer: Timer::Team(TeamTimer::EndGame),
            });
        } else {
            self.end_game(now_ms, out);
        }
    }

    fn reward(&mut self, out: &mut Vec<Effect>) {
        if self.state != TeamState::InGame || self.rewarded {
            return;
        }
        let r = self.rules;
        let (red_count, blue_count, red_gp, blue_gp, winner) = if self.team_count(RED) == 0 {
            (0, r.lose_signet, 0, 6, BLUE)
        } else if self.team_count(BLUE) == 0 {
            (r.lose_signet, 0, 6, 0, RED)
        } else if self.score[0] > self.score[1] {
            (r.win_signet, r.lose_signet, 25, 6, RED)
        } else if self.score[0] < self.score[1] {
            (r.lose_signet, r.win_signet, 6, 25, BLUE)
        } else {
            (r.draw_signet, r.draw_signet, 6, 6, INVALID)
        };
        if r.character_rank && winner != INVALID {
            if r.is_guild {
                if let Some(first) = self.team_slots(winner).first() {
                    out.push(Effect::GuildRankBasis { slot: *first });
                }
            } else {
                let rated = match r.rule {
                    Rule::DeathMatch => self.team_slots(winner).to_vec(),
                    Rule::GodMatch => self.without_penalty(winner),
                };
                for s in rated {
                    out.push(Effect::RankBasis { slot: s });
                }
            }
        }
        for (team, count, gp) in [(RED, red_count, red_gp), (BLUE, blue_count, blue_gp)] {
            for s in self.without_penalty(team) {
                if count > 0 {
                    out.push(Effect::Reward { slot: s, count });
                }
                out.push(Effect::Story {
                    slot: s,
                    success: winner == team,
                });
                if r.is_guild {
                    out.push(Effect::GuildGp { slot: s, gp });
                }
                if winner == team && r.has_event_reward {
                    out.push(Effect::EventReward { slot: s });
                }
            }
            if r.rule == Rule::GodMatch {
                for s in self.team_slots(team) {
                    if self.penalty.contains(s) {
                        out.push(Effect::Notice {
                            slot: *s,
                            format: "GameUI_Heroes_PvPReward_Panalty",
                        });
                    }
                }
            }
        }
        self.rewarded = true;
    }

    fn end_game(&mut self, now_ms: i64, out: &mut Vec<Effect>) {
        if self.state == TeamState::GameEnded {
            return;
        }
        self.reward(out);
        self.change_state(TeamState::GameEnded, now_ms);
        out.push(Effect::EndConfirm);
        out.push(Effect::Schedule {
            delay_ms: 32_000,
            timer: Timer::Team(TeamTimer::ForceRemove),
        });
        let mut text = format!("{},{}", self.rules.win_signet, self.rules.lose_signet);
        for m in self.members.values() {
            text.push_str(&format!(",{},0", m.name));
        }
        out.push(Effect::Command(cmd::END_GAME, text));
    }

    /// OnReport of the host.
    pub fn report(&mut self, event: i32, subject: i32, object: i32, now_ms: i64) -> Vec<Effect> {
        let mut out = Vec::new();
        if event == report::HOSTED {
            if self.state == TeamState::InGame {
                self.change_state(TeamState::WaitingRoom, now_ms);
            }
            return out;
        }
        if self.state != TeamState::InGame || self.rewarded {
            return out;
        }
        let slot = |v: i32| {
            usize::try_from(v)
                .ok()
                .filter(|s| self.members.contains_key(s))
        };
        match event {
            report::KILL => {
                let (Some(subj), Some(obj)) = (slot(subject), slot(object)) else {
                    return out;
                };
                let (subj_team, obj_team) = (self.team_of(subj), self.team_of(obj));
                let Some(obj_side) = side(obj_team) else {
                    return out;
                };
                match self.rules.rule {
                    Rule::DeathMatch => {
                        if self.stats.get(&obj).is_some_and(|s| s.dead) {
                            return out;
                        }
                        if subj_team != obj_team
                            && let Some(st) = self.stats.get_mut(&subj)
                        {
                            st.kill += 1;
                        }
                        if let Some(st) = self.stats.get_mut(&obj) {
                            st.death += 1;
                            st.dead = true;
                        }
                        self.count[obj_side] -= 1;
                        if self.count[obj_side] <= 0 {
                            self.end_round(now_ms, &mut out);
                        }
                    }
                    Rule::GodMatch => {
                        if subj_team != obj_team
                            && let Some(st) = self.stats.get_mut(&subj)
                        {
                            st.kill += 1;
                        }
                        if let Some(st) = self.stats.get_mut(&obj) {
                            st.death += 1;
                            st.dead = true;
                        }
                        if self.count[obj_side] <= 0 {
                            self.end_round(now_ms, &mut out);
                        }
                    }
                }
            }
            report::REVIVE if self.rules.rule == Rule::DeathMatch => {
                let Some(obj) = slot(object) else { return out };
                let Some(obj_side) = side(self.team_of(obj)) else {
                    return out;
                };
                if let Some(st) = self.stats.get_mut(&obj)
                    && st.dead
                {
                    st.dead = false;
                    self.count[obj_side] += 1;
                }
            }
            report::GOD_KILL if self.rules.rule == Rule::GodMatch => {
                if let Some(s) = side(object)
                    && self.god_alive[s]
                {
                    self.god_alive[s] = false;
                    self.end_round(now_ms, &mut out);
                }
            }
            _ => {}
        }
        out
    }

    /// PvpPlayTeam.RemovePlayer of a slot: OnRemovePlayer (the last of a team ends the game
    /// after the removal; else a death), then the slot leaves its team.
    pub fn remove_player(&mut self, slot: usize, now_ms: i64) -> Vec<Effect> {
        let mut out = Vec::new();
        let team = self.team_of(slot);
        if let Some(s) = side(team) {
            if self.team_count(team) == 1 {
                out.push(Effect::Schedule {
                    delay_ms: 0,
                    timer: Timer::Team(TeamTimer::EndGame),
                });
            } else if !self.rewarded {
                let alive = self.stats.get(&slot).is_some_and(|st| !st.dead);
                let dies = match self.rules.rule {
                    Rule::DeathMatch => alive,
                    Rule::GodMatch => true,
                };
                if dies {
                    if let Some(st) = self.stats.get_mut(&slot) {
                        st.death += 1;
                        st.dead = true;
                    }
                    self.count[s] -= 1;
                    if self.count[s] <= 0 {
                        self.end_round(now_ms, &mut out);
                    }
                }
            }
        }
        self.red.retain(|x| *x != slot);
        self.blue.retain(|x| *x != slot);
        self.members.remove(&slot);
        self.penalty.remove(&slot);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(rule: Rule) -> Rules {
        Rules {
            rule,
            game_length: 600,
            wait_before_start: 60,
            wait_between_round: 10,
            round_to_win: if rule == Rule::DeathMatch { 2 } else { 1 },
            win_signet: 4,
            draw_signet: 2,
            lose_signet: 2,
            min_clear_time: 420,
            is_guild: false,
            has_event_reward: false,
            character_rank: true,
        }
    }
    fn game(rule: Rule) -> TeamGame {
        let mut g = TeamGame::new(rules(rule), 0);
        g.add_player(
            0,
            RED,
            Member {
                cid: 10,
                name: "Ann".to_owned(),
            },
        );
        g.add_player(
            1,
            BLUE,
            Member {
                cid: 20,
                name: "Bob".to_owned(),
            },
        );
        g
    }
    fn commands(out: &[Effect]) -> Vec<i32> {
        out.iter()
            .filter_map(|e| match e {
                Effect::Command(c, _) => Some(*c),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn deathmatch_runs_two_rounds_waits_min_clear_time_and_rewards() {
        let mut g = game(Rule::DeathMatch);
        assert_eq!(
            g.on_player_ready(true),
            vec![Effect::Schedule {
                delay_ms: 10_000,
                timer: Timer::Team(TeamTimer::AllReady)
            }]
        );
        let out = g.timer(TeamTimer::AllReady, 10_000);
        assert_eq!(commands(&out), [cmd::ALL_PLAYER_READY]);
        let out = g.timer(TeamTimer::StartGame, 70_000);
        assert_eq!(commands(&out), [cmd::START_GAME]);
        assert_eq!(g.state(), TeamState::InGame);
        // Round 1: Ann kills Bob.
        let out = g.report(report::KILL, 0, 1, 80_000);
        assert_eq!(commands(&out), [cmd::END_ROUND]);
        g.timer(TeamTimer::StartRound, 90_000);
        // Round 2 too fast: the end waits for MinClearTime.
        let out = g.report(report::KILL, 0, 1, 100_000);
        assert!(matches!(out[0], Effect::Telepathy { .. }));
        let out = g.timer(TeamTimer::EndGame, 490_000);
        assert!(out.contains(&Effect::Reward { slot: 0, count: 4 }));
        assert!(out.contains(&Effect::Reward { slot: 1, count: 2 }));
        assert!(out.contains(&Effect::RankBasis { slot: 0 }));
        assert!(out.contains(&Effect::EndConfirm));
        assert_eq!(g.state(), TeamState::GameEnded);
    }

    #[test]
    fn godmatch_ends_on_a_god_kill_and_skips_the_penalized() {
        let mut g = game(Rule::GodMatch);
        g.timer(TeamTimer::StartGame, 0);
        g.penalty(1);
        // The red god dies: blue wins its only round (RoundToWin 1) after MinClearTime.
        let out = g.report(report::GOD_KILL, 1, RED, 500_000);
        assert!(out.contains(&Effect::Reward { slot: 0, count: 2 }));
        assert!(
            !out.iter()
                .any(|e| matches!(e, Effect::Reward { slot: 1, .. }))
        );
        assert!(out.contains(&Effect::Notice {
            slot: 1,
            format: "GameUI_Heroes_PvPReward_Panalty"
        }));
        // A team left empty ends the game after the removal.
        let mut g = game(Rule::GodMatch);
        g.timer(TeamTimer::StartGame, 0);
        let out = g.remove_player(1, 1000);
        assert_eq!(
            out,
            vec![Effect::Schedule {
                delay_ms: 0,
                timer: Timer::Team(TeamTimer::EndGame)
            }]
        );
        let out = g.timer(TeamTimer::EndGame, 1000);
        assert!(out.contains(&Effect::Reward { slot: 0, count: 2 }));
    }
}
