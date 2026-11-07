//! ArenaGame and ArenaTeam of the PvpService (B5 PV3a) as a state machine: one persistent
//! play per channel whose players queue for PairOfPlayer bouts of TeamSize against TeamSize
//! (GameWaitingQueue, TeamSlotInfo), the WaitingRoom → WaitingMatching → WaitingStart →
//! InGame → GameEnded cycle, rounds and the bout rewards. Players are play slots; every
//! effect (a command to the host, a dialog, a timer, a reward) is returned to the caller.
//! The source's Scheduler jobs are `Timer`s; a job the source enqueues runs after the
//! current step, so it is a timer of no delay.
use std::collections::BTreeMap;

/// PvpTeamID.
pub const INVALID: i32 = -1;
pub const BLUE: i32 = 2;
pub const RED: i32 = 3;

/// PvpCommandType values used by the Arena.
mod cmd {
    pub const START_GAME: i32 = 0;
    pub const END_GAME: i32 = 2;
    pub const ALL_PLAYER_READY: i32 = 3;
    pub const START_ROUND: i32 = 4;
    pub const END_ROUND: i32 = 5;
    pub const REGISTERED_PLAYER_QUEUE: i32 = 6;
    pub const REGISTERED_PLAYER_GAME: i32 = 7;
    pub const UNREGISTERED_PLAYER_QUEUE: i32 = 8;
    pub const UNREGISTERED_PLAYER_GAME: i32 = 9;
    pub const STATE_CHANGED: i32 = 10;
    pub const ARENA_STARTING_PLAYER_INFO: i32 = 11;
    pub const ARENA_END_GAME: i32 = 12;
}
/// PvpReportType values the Arena handles.
mod report {
    pub const HOSTED: i32 = 0;
    pub const KILL: i32 = 4;
    pub const REVIVE: i32 = 6;
    pub const WIN: i32 = 8;
    pub const DRAW: i32 = 9;
}

const FAIL: &str = "0,0,GameUI_Heroes_Pvp_Arena_StateChanged_Mathcing_Fail";

/// PvpGameState of the Arena cycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameState {
    WaitingRoom,
    WaitingMatching,
    WaitingStart,
    InGame,
    GameEnded,
}

/// A Scheduler job of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timer {
    ChangeState(GameState),
    StartGame,
    StartRound(usize),
    EndRound(usize),
    /// EndGame(index) enqueued by a leaving player.
    EndGame(usize),
    /// EndGame() of the whole cycle (GameEnded).
    EndCycle,
    /// A job of a DeathMatch or GodMatch game (`pvp_team`).
    Team(super::pvp_team::TeamTimer),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// PvpPlay.SendCommand (to the host); the arg keeps its leading/trailing commas trimmed
    /// by the caller.
    Command(i32, String),
    /// SystemMessage(Dialog) to the player of a slot.
    Dialog {
        slot: usize,
        format: &'static str,
    },
    Schedule {
        delay_ms: u64,
        timer: Timer,
    },
    /// PvpPlayer.GiveRewardItem(RewardItem, count).
    Reward {
        slot: usize,
        count: i32,
    },
    /// GiveRewardItem(EventRewardItem, 1).
    EventReward {
        slot: usize,
    },
    /// UpdateRankBasis_Pvp(CID, PvpMode): score 1 to RankID "Pvp_" + mode.
    RankBasis {
        slot: usize,
    },
    /// PvpPlay.NotifyStory(player, mode, isSuccess).
    Story {
        slot: usize,
        success: bool,
    },
    /// PvpResultLog(CID, mode, win, lose, successive); a winner then gets its PvpResultInfo
    /// (UpdateArenaWinCount, ArenaResultInfoMessage).
    Result {
        slot: usize,
        win: bool,
        successive: i32,
    },
    /// SystemMessage(Telepathy) to every player of the play.
    Telepathy {
        format: &'static str,
        args: Vec<String>,
    },
    /// SystemMessage(System) to the player of a slot.
    Notice {
        slot: usize,
        format: &'static str,
    },
    /// GiveGuildGP(ByPVP) of the slot's guild member.
    GuildGp {
        slot: usize,
        gp: i32,
    },
    /// RankBasis "Pvp_" + mode of the slot's guild (IsGuild).
    GuildRankBasis {
        slot: usize,
    },
    /// Play.GameEndConfirm: every player confirms with PvpUnregister, or all leave after 32 s.
    EndConfirm,
}

/// The ArenaConfig and FeatureMatrix values of the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    pub pair_of_player: usize,
    pub team_size: usize,
    pub max_size: usize,
    pub game_length: i32,
    pub wait_between_round: i32,
    pub round_to_win: i32,
    pub win_signet: i32,
    pub lose_signet: i32,
    /// SamePlayerRewardCoolDown, minutes.
    pub reward_cool_down: i32,
    pub has_event_reward: bool,
    /// FeatureMatrix ArenaDoubleEvent.
    pub double_event: bool,
    /// FeatureMatrix CharacterRank.
    pub character_rank: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct Stat {
    kill: i32,
    death: i32,
    dead: bool,
}
impl Stat {
    fn data(&self) -> String {
        format!(
            "{},{},{}",
            self.kill,
            self.death,
            if self.dead { 0 } else { 1 }
        )
    }
}

#[derive(Clone, Debug, Default)]
struct Bout {
    red: Vec<usize>,
    blue: Vec<usize>,
    score_red: i32,
    score_blue: i32,
    count_red: i32,
    count_blue: i32,
    starting: bool,
    round_start_ms: i64,
}

/// PvpGameInfo: TeamInfo and PlayerInfo rows.
pub type GameInfo = (Vec<(i32, String)>, Vec<(i32, String)>);

/// A play slot's player: CID and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub cid: i64,
    pub name: String,
}

pub struct Arena {
    rules: Rules,
    state: GameState,
    changed_ms: i64,
    bouts: Vec<Bout>,
    queue: Vec<Vec<usize>>,
    successive: Vec<i32>,
    stats: BTreeMap<usize, Stat>,
    /// LastRewardTimeDict: (winner CID, loser CID) → milliseconds.
    last_reward: BTreeMap<(i64, i64), i64>,
    members: Vec<Option<Member>>,
}

impl Arena {
    pub fn new(rules: Rules, now_ms: i64) -> Self {
        Self {
            state: GameState::WaitingRoom,
            changed_ms: now_ms,
            bouts: vec![Bout::default(); rules.pair_of_player],
            queue: vec![Vec::new(); rules.pair_of_player],
            successive: vec![0; rules.max_size],
            stats: BTreeMap::new(),
            last_reward: BTreeMap::new(),
            members: vec![None; rules.max_size],
            rules,
        }
    }
    pub fn state(&self) -> GameState {
        self.state
    }
    /// ArenaTeam.AddPlayer's slot.
    pub fn add_player(&mut self, slot: usize, member: Member) {
        if let Some(s) = self.members.get_mut(slot) {
            *s = Some(member);
            self.stats.insert(slot, Stat::default());
        }
    }
    fn name(&self, slot: usize) -> String {
        self.members
            .get(slot)
            .and_then(|m| m.as_ref())
            .map(|m| m.name.clone())
            .unwrap_or_default()
    }
    fn present(&self, slot: usize) -> bool {
        self.members.get(slot).is_some_and(Option::is_some)
    }
    fn cid(&self, slot: usize) -> Option<i64> {
        self.members
            .get(slot)
            .and_then(|m| m.as_ref())
            .map(|m| m.cid)
    }
    /// ArenaTeam.GetPlayerTeamID.
    pub fn team_of(&self, slot: usize) -> i32 {
        for b in &self.bouts {
            if b.red.contains(&slot) {
                return RED;
            }
            if b.blue.contains(&slot) {
                return BLUE;
            }
        }
        INVALID
    }
    /// ArenaTeam.GetPlayerTeamIndex (the last bout holding the slot).
    fn index_of(&self, slot: usize) -> Option<usize> {
        self.bouts
            .iter()
            .rposition(|b| b.red.contains(&slot) || b.blue.contains(&slot))
    }
    fn side(&mut self, index: usize, team: i32) -> &mut Vec<usize> {
        if team == RED {
            &mut self.bouts[index].red
        } else {
            &mut self.bouts[index].blue
        }
    }
    fn count_team(&self, index: usize, team: i32) -> usize {
        match team {
            RED => self.bouts[index].red.len(),
            BLUE => self.bouts[index].blue.len(),
            _ => 0,
        }
    }
    /// GetTeamPlayers: the slots of a side that have a player.
    fn players(&self, index: usize, team: i32) -> Vec<usize> {
        let side = match team {
            RED => &self.bouts[index].red,
            BLUE => &self.bouts[index].blue,
            _ => return Vec::new(),
        };
        side.iter().copied().filter(|s| self.present(*s)).collect()
    }
    fn player_count(&mut self, index: usize, team: i32) -> &mut i32 {
        if team == RED {
            &mut self.bouts[index].count_red
        } else {
            &mut self.bouts[index].count_blue
        }
    }

    /// GameInfo: TeamInfo {-1: "1,{remaining}" in game, else "0, 0"} and the slots' stats.
    pub fn game_info(&self, now_ms: i64) -> GameInfo {
        let in_game = self.state == GameState::InGame;
        let team = if in_game {
            let passed = ((now_ms - self.changed_ms) / 1000) as i32;
            format!("1,{}", self.rules.game_length - passed)
        } else {
            "0, 0".to_owned()
        };
        let players = (0..self.members.len())
            .filter(|s| self.present(*s))
            .map(|s| {
                let text = if in_game {
                    self.stats.get(&s).copied().unwrap_or_default().data()
                } else {
                    "0,0,0,0".to_owned()
                };
                (s as i32, text)
            })
            .collect();
        (vec![(-1, team)], players)
    }

    fn change_state(&mut self, state: GameState, now_ms: i64, out: &mut Vec<Effect>) {
        self.state = state;
        self.changed_ms = now_ms;
        match state {
            GameState::WaitingMatching => {
                if self.check_next_state() {
                    out.push(Effect::Command(
                        cmd::STATE_CHANGED,
                        "1,1,GameUI_Heroes_Pvp_Arena_StateChanged_WaitingMathcing".to_owned(),
                    ));
                    out.push(Effect::Schedule {
                        delay_ms: 5000,
                        timer: Timer::ChangeState(GameState::WaitingStart),
                    });
                } else {
                    out.push(Effect::Command(cmd::STATE_CHANGED, FAIL.to_owned()));
                    self.change_state(GameState::WaitingRoom, now_ms, out);
                }
            }
            GameState::WaitingStart => {
                if self.check_next_state() && self.on_all_player_ready(10, out) {
                    out.push(Effect::Command(
                        cmd::STATE_CHANGED,
                        "2,1,GameUI_Heroes_Pvp_Arena_StateChanged_WaitingStart".to_owned(),
                    ));
                } else {
                    out.push(Effect::Command(cmd::STATE_CHANGED, FAIL.to_owned()));
                    self.change_state(GameState::WaitingRoom, now_ms, out);
                }
            }
            GameState::InGame => out.push(Effect::Command(
                cmd::STATE_CHANGED,
                "3,1,GameUI_Heroes_Pvp_Arena_StateChanged_InGame".to_owned(),
            )),
            GameState::GameEnded => {
                self.set_battle_game(out);
                out.push(Effect::Command(
                    cmd::STATE_CHANGED,
                    "7,1,GameUI_Heroes_Pvp_Arena_StateChanged_GameEnded".to_owned(),
                ));
                out.push(Effect::Schedule {
                    delay_ms: 15000,
                    timer: Timer::ChangeState(GameState::WaitingMatching),
                });
            }
            GameState::WaitingRoom => {}
        }
    }

    /// CheckNextState: a bout whose sides and queue reach 2 × TeamSize.
    fn check_next_state(&self) -> bool {
        matches!(
            self.state,
            GameState::WaitingRoom
                | GameState::WaitingMatching
                | GameState::WaitingStart
                | GameState::InGame
        ) && (0..self.bouts.len()).any(|i| {
            self.bouts[i].red.len() + self.bouts[i].blue.len() + self.queue[i].len()
                >= self.rules.team_size * 2
        })
    }

    fn set_battle_game(&mut self, out: &mut Vec<Effect>) -> bool {
        let mut any = false;
        for i in 0..self.bouts.len() {
            if self.set_battle_game_at(i, out) {
                any = true;
            }
        }
        any
    }

    /// SetBattleGameByTeamIndex: the queue's first players fill the red then the blue side.
    fn set_battle_game_at(&mut self, index: usize, out: &mut Vec<Effect>) -> bool {
        let size = self.rules.team_size;
        let need_red = size.saturating_sub(self.count_team(index, RED));
        let red = (self.queue[index].len() >= need_red)
            .then(|| self.queue[index].drain(..need_red).collect::<Vec<_>>());
        let need_blue = size.saturating_sub(self.count_team(index, BLUE));
        let blue = (self.queue[index].len() >= need_blue)
            .then(|| self.queue[index].drain(..need_blue).collect::<Vec<_>>());
        if let Some(red) = red
            && self.add_battle_player(index, &red, RED, out)
        {
            for s in &red {
                self.remove_waiting(*s, out);
            }
            if let Some(blue) = blue
                && self.add_battle_player(index, &blue, BLUE, out)
            {
                for s in &blue {
                    self.remove_waiting(*s, out);
                }
                return self.count_team(index, RED) + self.count_team(index, BLUE) == size * 2;
            }
        }
        false
    }

    /// ArenaTeam.AddBattlePlayer.
    fn add_battle_player(
        &mut self,
        index: usize,
        slots: &[usize],
        team: i32,
        out: &mut Vec<Effect>,
    ) -> bool {
        if index >= self.bouts.len()
            || self.count_team(index, team) + slots.len() != self.rules.team_size
        {
            return false;
        }
        let mut text = String::new();
        for s in slots {
            self.side(index, team).push(*s);
            text.push(',');
            text.push_str(&self.name(*s));
        }
        if !text.is_empty() {
            out.push(Effect::Command(
                cmd::REGISTERED_PLAYER_GAME,
                format!("{index},{team},GameUI_Heroes_Pvp_Arena_Registerd_Game{text}"),
            ));
        }
        true
    }

    /// ArenaTeam.RemoveBattlePlayer.
    fn remove_battle_player(
        &mut self,
        index: usize,
        slot: usize,
        team: i32,
        out: &mut Vec<Effect>,
    ) -> bool {
        if team == INVALID || index >= self.bouts.len() {
            return false;
        }
        let side = self.side(index, team);
        let Some(pos) = side.iter().position(|s| *s == slot) else {
            return false;
        };
        side.remove(pos);
        out.push(Effect::Command(
            cmd::UNREGISTERED_PLAYER_GAME,
            format!(
                "{index},{team},GameUI_Heroes_Pvp_Arena_Unregisterd_Game,{}",
                self.name(slot)
            ),
        ));
        true
    }

    /// RemoveWaitingQueueGame.
    fn remove_waiting(&mut self, slot: usize, out: &mut Vec<Effect>) -> bool {
        for i in 0..self.queue.len() {
            if let Some(pos) = self.queue[i].iter().position(|s| *s == slot) {
                self.queue[i].remove(pos);
                out.push(Effect::Command(
                    cmd::UNREGISTERED_PLAYER_QUEUE,
                    format!(
                        "{i},GameUI_Heroes_Pvp_Arena_Unregisterd_Queue,{}",
                        self.name(slot)
                    ),
                ));
                return true;
            }
        }
        false
    }

    /// AddWaitingQueueGame (PvpRegisterGameWaitingQueue); false is the processor's Unknown.
    pub fn add_waiting_queue(
        &mut self,
        index: usize,
        slot: usize,
        now_ms: i64,
    ) -> Option<Vec<Effect>> {
        if index >= self.queue.len() || !self.present(slot) {
            return None;
        }
        let mut out = Vec::new();
        if let Some(s) = self.successive.get_mut(slot) {
            *s = 0;
        }
        if let Some(at) = self.index_of(slot) {
            let team = self.team_of(slot);
            let in_target = match team {
                RED => self.bouts[index].red.contains(&slot),
                BLUE => self.bouts[index].blue.contains(&slot),
                _ => false,
            };
            if in_target {
                self.remove_battle_player(at, slot, team, &mut out);
                self.remove_waiting(slot, &mut out);
            } else {
                self.remove_waiting(slot, &mut out);
                self.remove_battle_player(at, slot, team, &mut out);
            }
        } else {
            self.remove_waiting(slot, &mut out);
        }
        self.queue[index].push(slot);
        if self.state == GameState::WaitingRoom && self.check_next_state() {
            self.change_state(GameState::WaitingMatching, now_ms, &mut out);
        }
        out.push(Effect::Command(
            cmd::REGISTERED_PLAYER_QUEUE,
            format!(
                "{index},GameUI_Heroes_Pvp_Arena_Registerd_Waiting_Queue,{}",
                self.name(slot)
            ),
        ));
        if self.state != GameState::WaitingStart {
            self.set_battle_game_at(index, &mut out);
        }
        Some(out)
    }

    /// PvpUnregisterGameWaitingQueueProcessor.
    pub fn remove_waiting_queue(&mut self, slot: usize) -> Vec<Effect> {
        let mut out = Vec::new();
        if let Some(at) = self.index_of(slot) {
            let team = self.team_of(slot);
            self.remove_battle_player(at, slot, team, &mut out);
        }
        self.remove_waiting(slot, &mut out);
        out
    }

    fn send_starting_player_info(&self, out: &mut Vec<Effect>) {
        let mut text = String::new();
        for i in 0..self.bouts.len() {
            for team in [RED, BLUE] {
                for s in self.players(i, team) {
                    text.push_str(&format!(
                        ",{i},{},{team},{}",
                        self.name(s),
                        self.successive.get(s).copied().unwrap_or(0)
                    ));
                }
            }
        }
        out.push(Effect::Command(cmd::ARENA_STARTING_PLAYER_INFO, text));
    }

    fn on_all_player_ready(&mut self, delay: u64, out: &mut Vec<Effect>) -> bool {
        if !self.set_battle_game(out) {
            return false;
        }
        self.send_starting_player_info(out);
        out.push(Effect::Command(cmd::ALL_PLAYER_READY, delay.to_string()));
        out.push(Effect::Schedule {
            delay_ms: delay * 1000,
            timer: Timer::StartGame,
        });
        true
    }

    fn start_game(&mut self, now_ms: i64, out: &mut Vec<Effect>) {
        if self.state == GameState::WaitingStart {
            self.state = GameState::InGame;
            for b in &mut self.bouts {
                b.score_red = 0;
                b.score_blue = 0;
            }
            self.all_start_round(now_ms, out);
        } else {
            self.change_state(GameState::WaitingRoom, now_ms, out);
            out.push(Effect::Command(cmd::STATE_CHANGED, FAIL.to_owned()));
        }
    }

    fn all_start_round(&mut self, now_ms: i64, out: &mut Vec<Effect>) {
        if !self.members.iter().any(Option::is_some) {
            out.push(Effect::Command(cmd::STATE_CHANGED, FAIL.to_owned()));
            self.change_state(GameState::WaitingRoom, now_ms, out);
            return;
        }
        let mut none = true;
        for i in 0..self.bouts.len() {
            let (red, blue) = (
                self.count_team(i, RED) as i32,
                self.count_team(i, BLUE) as i32,
            );
            let b = &mut self.bouts[i];
            b.count_red = red;
            b.count_blue = blue;
            if red != 0 && blue != 0 {
                none = false;
                b.starting = true;
                b.round_start_ms = now_ms;
                out.push(Effect::Schedule {
                    delay_ms: (self.rules.game_length.max(0) as u64) * 1000 + 15000,
                    timer: Timer::EndRound(i),
                });
            }
        }
        if none {
            self.change_state(GameState::WaitingRoom, now_ms, out);
            out.push(Effect::Command(cmd::STATE_CHANGED, FAIL.to_owned()));
            return;
        }
        for s in self.stats.values_mut() {
            s.dead = false;
        }
        out.push(Effect::Command(
            cmd::START_GAME,
            self.rules.game_length.to_string(),
        ));
        self.change_state(GameState::InGame, now_ms, out);
    }

    fn start_round(&mut self, index: usize, now_ms: i64, out: &mut Vec<Effect>) {
        if index >= self.bouts.len() {
            return;
        }
        let (red, blue) = (
            self.count_team(index, RED) as i32,
            self.count_team(index, BLUE) as i32,
        );
        self.bouts[index].count_red = red;
        self.bouts[index].count_blue = blue;
        for team in [RED, BLUE] {
            for s in self.players(index, team) {
                if let Some(st) = self.stats.get_mut(&s) {
                    st.dead = false;
                }
            }
        }
        out.push(Effect::Command(
            cmd::START_ROUND,
            format!("{},{index}", self.rules.game_length),
        ));
        self.bouts[index].round_start_ms = now_ms;
        self.bouts[index].starting = true;
        out.push(Effect::Schedule {
            delay_ms: (self.rules.game_length.max(0) as u64) * 1000 + 200,
            timer: Timer::EndRound(index),
        });
    }

    fn end_round(&mut self, index: usize, now_ms: i64, out: &mut Vec<Effect>) {
        if self.state != GameState::InGame
            || index >= self.bouts.len()
            || !self.bouts[index].starting
        {
            return;
        }
        let length_ms = i64::from(self.rules.game_length) * 1000;
        let b = &mut self.bouts[index];
        if b.count_red == 0 {
            b.score_blue += 1;
        } else if b.count_blue == 0 {
            b.score_red += 1;
        } else {
            if now_ms - b.round_start_ms <= length_ms {
                return;
            }
            b.score_red += 1;
            b.score_blue += 1;
        }
        if b.score_red >= self.rules.round_to_win || b.score_blue >= self.rules.round_to_win {
            self.end_game(index, now_ms, out);
            return;
        }
        out.push(Effect::Command(
            cmd::END_ROUND,
            format!("{},{index}", self.rules.wait_between_round),
        ));
        out.push(Effect::Schedule {
            delay_ms: (self.rules.wait_between_round.max(0) as u64) * 1000,
            timer: Timer::StartRound(index),
        });
    }

    fn end_game(&mut self, index: usize, now_ms: i64, out: &mut Vec<Effect>) {
        let text = self
            .members
            .iter()
            .flatten()
            .map(|m| format!("{},0", m.name))
            .collect::<Vec<_>>()
            .join(",");
        out.push(Effect::Command(cmd::END_GAME, text));
        self.reward(index, now_ms, out);
        if index < self.bouts.len() {
            self.bouts[index].starting = false;
        }
        if self.bouts.iter().all(|b| !b.starting) {
            out.push(Effect::Schedule {
                delay_ms: 4000,
                timer: Timer::EndCycle,
            });
        }
    }

    fn reward(&mut self, index: usize, now_ms: i64, out: &mut Vec<Effect>) {
        if self.state != GameState::InGame || index >= self.bouts.len() {
            return;
        }
        let red = self.players(index, RED).first().copied();
        let blue = self.players(index, BLUE).first().copied();
        let b = &self.bouts[index];
        let r = &self.rules;
        let (mut red_count, mut blue_count, winner) = match (red, blue) {
            (None, _) => (0, r.win_signet, BLUE),
            (_, None) => (r.win_signet, 0, RED),
            _ if b.score_red > b.score_blue => (r.win_signet, r.lose_signet, RED),
            _ if b.score_red < b.score_blue => (r.lose_signet, r.win_signet, BLUE),
            _ => (0, 0, INVALID),
        };
        if r.double_event {
            red_count *= 2;
            blue_count *= 2;
        }
        let pair = match winner {
            RED => Some((red, blue)),
            BLUE => Some((blue, red)),
            _ => None,
        };
        if let Some((w, l)) = pair {
            let key = (
                w.and_then(|s| self.cid(s)).unwrap_or(-1),
                l.and_then(|s| self.cid(s)).unwrap_or(-1),
            );
            let cool = i64::from(r.reward_cool_down) * 60_000;
            match self.last_reward.get(&key) {
                Some(at) if now_ms <= at + cool => {
                    red_count = 0;
                    blue_count = 0;
                }
                _ => {
                    self.last_reward.insert(key, now_ms);
                }
            }
        }
        if r.character_rank && winner != INVALID {
            for s in self.players(index, winner) {
                out.push(Effect::RankBasis { slot: s });
            }
        }
        for s in self.players(index, winner) {
            if let Some(c) = self.successive.get_mut(s) {
                *c += 1;
            }
        }
        for (team, count) in [(RED, red_count), (BLUE, blue_count)] {
            for s in self.players(index, team) {
                if count > 0 {
                    out.push(Effect::Reward { slot: s, count });
                }
                out.push(Effect::Story {
                    slot: s,
                    success: winner == team,
                });
                if winner == team && self.rules.has_event_reward {
                    out.push(Effect::EventReward { slot: s });
                }
                out.push(Effect::Result {
                    slot: s,
                    win: winner == team,
                    successive: self.successive.get(s).copied().unwrap_or(0),
                });
            }
        }
        let losers: Vec<usize> = if winner == INVALID {
            let mut all = self.players(index, RED);
            all.extend(self.players(index, BLUE));
            all
        } else {
            self.players(index, if winner == BLUE { RED } else { BLUE })
        };
        for s in &losers {
            if let Some(c) = self.successive.get_mut(*s) {
                *c = 0;
            }
        }
        for s in losers {
            let team = self.team_of(s);
            if self.remove_battle_player(index, s, team, out) {
                if let Some(c) = self.successive.get_mut(s) {
                    *c = 0;
                }
                out.push(Effect::Dialog {
                    slot: s,
                    format: "GameUI_Heroes_Arena_Game_Lose",
                });
            }
        }
        let (win_count, lose_count) = if winner == RED {
            (red_count, blue_count)
        } else {
            (blue_count, red_count)
        };
        out.push(Effect::Command(
            cmd::ARENA_END_GAME,
            format!("{win_count},{lose_count},{index},{winner}"),
        ));
        let b = &mut self.bouts[index];
        b.score_red = 0;
        b.score_blue = 0;
    }

    /// A Scheduler job firing.
    pub fn timer(&mut self, timer: Timer, now_ms: i64) -> Vec<Effect> {
        let mut out = Vec::new();
        match timer {
            Timer::ChangeState(s) => self.change_state(s, now_ms, &mut out),
            Timer::StartGame => self.start_game(now_ms, &mut out),
            Timer::StartRound(i) => self.start_round(i, now_ms, &mut out),
            Timer::EndRound(i) => self.end_round(i, now_ms, &mut out),
            Timer::EndGame(i) => self.end_game(i, now_ms, &mut out),
            Timer::EndCycle => self.change_state(GameState::GameEnded, now_ms, &mut out),
            Timer::Team(_) => {}
        }
        out
    }

    /// OnReport of the host.
    pub fn report(&mut self, event: i32, subject: i32, object: i32, now_ms: i64) -> Vec<Effect> {
        let mut out = Vec::new();
        let slot = |v: i32| usize::try_from(v).ok().filter(|s| self.present(*s));
        if event == report::HOSTED {
            if self.state == GameState::InGame {
                self.change_state(GameState::WaitingRoom, now_ms, &mut out);
            }
            return out;
        }
        if self.state != GameState::InGame {
            return out;
        }
        let (subj, obj) = (slot(subject), slot(object));
        match event {
            report::KILL => {
                let Some(obj) = obj else { return out };
                if self.stats.get(&obj).is_some_and(|s| s.dead) {
                    return out;
                }
                let Some(index) = self.index_of(obj) else {
                    return out;
                };
                if let Some(subj) = subj
                    && subj != obj
                    && let Some(st) = self.stats.get_mut(&subj)
                {
                    st.kill += 1;
                }
                if let Some(st) = self.stats.get_mut(&obj) {
                    st.death += 1;
                    st.dead = true;
                }
                let team = self.team_of(obj);
                let count = self.player_count(index, team);
                *count -= 1;
                if *count <= 0 {
                    self.end_round(index, now_ms, &mut out);
                }
            }
            report::REVIVE => {
                let Some(obj) = obj else { return out };
                let Some(st) = self.stats.get_mut(&obj) else {
                    return out;
                };
                if !st.dead {
                    return out;
                }
                st.dead = false;
                if let Some(index) = self.index_of(obj) {
                    let team = self.team_of(obj);
                    *self.player_count(index, team) += 1;
                }
            }
            report::WIN => {
                let (Some(_), Some(obj)) = (subj, obj) else {
                    return out;
                };
                if let Some(index) = self.index_of(obj) {
                    let team = self.team_of(obj);
                    *self.player_count(index, team) -= 1;
                    self.end_round(index, now_ms, &mut out);
                }
            }
            report::DRAW => {
                let (Some(_), Some(obj)) = (subj, obj) else {
                    return out;
                };
                if let Some(index) = self.index_of(obj) {
                    self.end_round(index, now_ms, &mut out);
                }
            }
            _ => {}
        }
        out
    }

    /// ArenaTeam.RemovePlayer of a slot: OnRemovePlayer (the queue, the successive wins, a
    /// bout's end or death in game, else the bout's refill), then the slot and its side.
    pub fn remove_player(&mut self, slot: usize) -> Vec<Effect> {
        let mut out = Vec::new();
        if !self.present(slot) {
            return out;
        }
        self.remove_waiting(slot, &mut out);
        if let Some(s) = self.successive.get_mut(slot) {
            *s = 0;
        }
        let index = self.index_of(slot);
        if let Some(index) = index {
            let team = self.team_of(slot);
            if self.state == GameState::InGame {
                if self.count_team(index, team) == 1 {
                    out.push(Effect::Schedule {
                        delay_ms: 0,
                        timer: Timer::EndGame(index),
                    });
                } else if self.stats.get(&slot).is_some_and(|s| !s.dead) {
                    if let Some(st) = self.stats.get_mut(&slot) {
                        st.death += 1;
                        st.dead = true;
                    }
                    let count = self.player_count(index, team);
                    *count -= 1;
                    if *count <= 0 {
                        out.push(Effect::Schedule {
                            delay_ms: 0,
                            timer: Timer::EndRound(index),
                        });
                    }
                }
            } else if self.state != GameState::WaitingStart {
                self.set_battle_game_at(index, &mut out);
            }
        }
        if let Some(m) = self.members.get_mut(slot) {
            *m = None;
        }
        if let Some(index) = index {
            let team = self.team_of(slot);
            self.remove_battle_player(index, slot, team, &mut out);
        }
        out
    }
    /// SuccessiveWinCount of a slot.
    pub fn successive(&self, slot: usize) -> i32 {
        self.successive.get(slot).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Rules {
        Rules {
            pair_of_player: 4,
            team_size: 1,
            max_size: 32,
            game_length: 90,
            wait_between_round: 15,
            round_to_win: 1,
            win_signet: 2,
            lose_signet: 1,
            reward_cool_down: 10,
            has_event_reward: false,
            double_event: false,
            character_rank: true,
        }
    }
    fn commands(out: &[Effect]) -> Vec<(i32, String)> {
        out.iter()
            .filter_map(|e| match e {
                Effect::Command(c, a) => Some((*c, a.clone())),
                _ => None,
            })
            .collect()
    }
    fn arena() -> Arena {
        let mut a = Arena::new(rules(), 0);
        a.add_player(
            0,
            Member {
                cid: 10,
                name: "Ann".to_owned(),
            },
        );
        a.add_player(
            1,
            Member {
                cid: 20,
                name: "Bob".to_owned(),
            },
        );
        a
    }

    #[test]
    fn two_queued_players_are_matched_started_and_rewarded() {
        let mut a = arena();
        let out = a.add_waiting_queue(0, 0, 0).unwrap();
        assert_eq!(
            commands(&out),
            [
                (
                    6,
                    "0,GameUI_Heroes_Pvp_Arena_Registerd_Waiting_Queue,Ann".to_owned()
                ),
                (
                    7,
                    "0,3,GameUI_Heroes_Pvp_Arena_Registerd_Game,Ann".to_owned()
                ),
            ]
        );
        let out = a.add_waiting_queue(0, 1, 0).unwrap();
        assert_eq!(a.state(), GameState::WaitingMatching);
        assert!(out.contains(&Effect::Schedule {
            delay_ms: 5000,
            timer: Timer::ChangeState(GameState::WaitingStart)
        }));
        assert!(commands(&out).contains(&(
            7,
            "0,2,GameUI_Heroes_Pvp_Arena_Registerd_Game,Bob".to_owned()
        )));
        let out = a.timer(Timer::ChangeState(GameState::WaitingStart), 5000);
        let c = commands(&out);
        assert_eq!(c[0], (11, ",0,Ann,3,0,0,Bob,2,0".to_owned()));
        assert_eq!(c[1], (3, "10".to_owned()));
        assert_eq!(
            c[2].1,
            "2,1,GameUI_Heroes_Pvp_Arena_StateChanged_WaitingStart"
        );
        let out = a.timer(Timer::StartGame, 15000);
        assert_eq!(a.state(), GameState::InGame);
        assert_eq!(commands(&out)[0], (0, "90".to_owned()));
        assert_eq!(a.game_info(15000).0, vec![(-1, "1,90".to_owned())]);
        // Ann kills Bob: the bout ends, Ann wins 2 signets, Bob loses 1 and leaves the bout.
        let out = a.report(report::KILL, 0, 1, 20000);
        assert!(out.contains(&Effect::Reward { slot: 0, count: 2 }));
        assert!(out.contains(&Effect::Reward { slot: 1, count: 1 }));
        assert!(out.contains(&Effect::RankBasis { slot: 0 }));
        assert!(out.contains(&Effect::Result {
            slot: 0,
            win: true,
            successive: 1
        }));
        assert!(out.contains(&Effect::Dialog {
            slot: 1,
            format: "GameUI_Heroes_Arena_Game_Lose"
        }));
        assert!(commands(&out).contains(&(12, "2,1,0,3".to_owned())));
        assert!(out.contains(&Effect::Schedule {
            delay_ms: 4000,
            timer: Timer::EndCycle
        }));
        assert_eq!(a.team_of(1), INVALID);
        assert_eq!(a.team_of(0), RED);
        // The same pair again within the cool down gets nothing.
        a.add_waiting_queue(0, 1, 21000);
        a.timer(Timer::EndCycle, 24000);
        a.timer(Timer::ChangeState(GameState::WaitingMatching), 39000);
        a.timer(Timer::ChangeState(GameState::WaitingStart), 44000);
        a.timer(Timer::StartGame, 54000);
        assert_eq!(a.state(), GameState::InGame);
        let out = a.report(report::KILL, 0, 1, 60000);
        assert!(!out.iter().any(|e| matches!(e, Effect::Reward { .. })));
        assert!(out.contains(&Effect::Result {
            slot: 0,
            win: true,
            successive: 2
        }));
    }

    #[test]
    fn a_timeout_with_both_alive_is_a_draw_and_a_leaver_loses() {
        let mut a = arena();
        a.add_waiting_queue(1, 0, 0);
        a.add_waiting_queue(1, 1, 0);
        a.timer(Timer::ChangeState(GameState::WaitingStart), 5000);
        a.timer(Timer::StartGame, 15000);
        // Too early: nothing; after GameLength both score and the bout ends in a draw.
        assert!(a.timer(Timer::EndRound(1), 20000).is_empty());
        let out = a.timer(Timer::EndRound(1), 15000 + 90_001);
        assert!(commands(&out).contains(&(12, "0,0,1,-1".to_owned())));
        assert_eq!((a.team_of(0), a.team_of(1)), (INVALID, INVALID));
        // A leaver in game: its bout ends after the removal and the other side wins.
        let mut a = arena();
        a.add_waiting_queue(0, 0, 0);
        a.add_waiting_queue(0, 1, 0);
        a.timer(Timer::ChangeState(GameState::WaitingStart), 5000);
        a.timer(Timer::StartGame, 15000);
        let out = a.remove_player(1);
        assert!(out.contains(&Effect::Schedule {
            delay_ms: 0,
            timer: Timer::EndGame(0)
        }));
        let out = a.timer(Timer::EndGame(0), 16000);
        assert!(out.contains(&Effect::Reward { slot: 0, count: 2 }));
        assert!(commands(&out).contains(&(12, "2,0,0,3".to_owned())));
    }
}
