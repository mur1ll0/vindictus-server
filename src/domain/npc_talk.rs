//! StoryEntity talk queries: TalkTo/SelectButton/ExecuteServerCommand, INpcCommand
//! DoChain with its server-command split, the client command pass-through and the
//! NPC conditions whose data is migrated. All source callbacks are synchronous, so the
//! continuation-passing structure is kept as explicit continuations.
use super::{
    feature_policy::FeaturePolicy,
    npc_script::{Cond, Node, NodeId, NpcScripts, Progress, SWITCH_BRANCHES},
};

/// ProcessCommandFunc.Process_* names: commands executed by the server.
pub const SERVER_COMMANDS: &[&str] = &[
    "rank_basis_carriage_move",
    "talk_to",
    "give_item",
    "give_item_failmail",
    "give_item_untradable",
    "take_item_enhanced",
    "take_item",
    "take_item_safe",
    "take_item_even_expired",
    "take_item_silent",
    "give_quest",
    "give_skill",
    "notice_title",
    "acquire_title",
    "give_title_by_episode",
    "clear_guide",
    "set_guide",
    "start_storyline",
    "reveal_storyline",
    "complete_storyline",
    "complete_storyline_and_delete",
    "delete_storyline",
    "goto_phase",
    "change_phase",
    "save_point",
    "set_timer",
    "set_token",
    "increase_token",
    "decrease_token",
    "give_storage",
    "give_ap",
    "unlock_skill",
    "give_exp",
    "ask_repair",
    "do_repair",
    "open_equipment_slot",
    "acquire_manufacture",
    "remove_manufacture",
    "levelup_manufacture",
    "acquire_manufacture_recipe",
    "select_vocation",
    "set_vocation_level",
    "complete_episode",
    "Count_Event",
];

/// NpcTalkEntity: Command, Arg1, Arg2 (null arguments become "").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TalkEntity {
    pub command: String,
    pub arg1: String,
    pub arg2: String,
}
fn entity(command: &str, arg1: Option<&str>, arg2: Option<&str>) -> TalkEntity {
    TalkEntity {
        command: command.to_owned(),
        arg1: arg1.unwrap_or("").to_owned(),
        arg2: arg2.unwrap_or("").to_owned(),
    }
}

/// Data the conditions read (DataCache, Item, Quest and StoryEntity state).
pub trait Facts {
    fn class(&self) -> i32;
    fn level(&self) -> i32;
    /// Titles.GetTitle(id).Status == Acquired.
    fn title_acquired(&self, id: i32) -> bool;
    /// Unexpired item count of an ItemClass.
    fn item_count(&self, class: &str) -> i32;
    /// QueryManufactureStatus: (grade, exp) when the manufacture is known.
    fn manufacture(&self, id: &str) -> Option<(i32, i32)>;
    /// QueryQuestState(questID).Progress.
    fn quest_progress(&self, id: &str) -> i32;
    /// ItemImpl.QueryRepairPrice(null, addAllRepairableItems: true).
    fn repair_price(&self) -> i32;
    /// Useable gold count (DestroyItem(gold, n) succeeds when n does not exceed it).
    fn gold(&self) -> i32;
}

/// Server command effects decided during the query and applied afterwards in one
/// transaction (their success was decided from the same Facts).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// ItemImpl.RepairItem(null, hideNotEnoughGold: false, addAllRepairableItems: true).
    RepairAll,
    /// ItemImpl.GiveItem(itemClass, num, OperationFail/ForceMail, Story*).
    GiveItem { class: String, count: i32 },
    /// ItemImpl.DestroyItem(itemClass, num, Story, includeExpired: false).
    TakeItem { class: String, count: i32 },
}

/// StoryEntity state a query reads and changes; the caller persists it when the
/// query succeeds (StoryProgress, StorySave, StoryVariable, StoryGuide).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoryState {
    pub progress: Vec<Progress>,
    /// StorySave rows: (NpcID, StoryLineID) and CmdID.
    pub saves: Vec<(String, String, String)>,
    /// StoryVariable rows in Entry order: StoryLineID, Key, Value (0 is never kept).
    pub variables: Vec<(String, String, i32)>,
    /// StoryGuide (GuideTarget, GuideMessage); None when the row does not exist.
    pub guide: Option<(String, String)>,
    /// Item count changes made by give/take_item in this query, by ItemClass.
    pub item_delta: Vec<(String, i64)>,
}
impl StoryState {
    fn variable(&self, line: &str, key: &str) -> i32 {
        self.variables
            .iter()
            .find(|(l, k, _)| l == line && k == key)
            .map_or(0, |v| v.2)
    }
}

/// What a query sends, in source order: immediate frontend messages, effects whose
/// Sync precedes the talk, the NpcTalk push and SyncStoryProgress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Output {
    Talk(Vec<TalkEntity>),
    /// SyncStoryProgress (StoryLinesMessage then NpcListMessage) of this progress.
    SyncStory(Vec<Progress>),
    /// UpdateStoryGuideMessage (SyncGuideInfo).
    Guide(String, String),
    /// StoryVariablesMessage after SetVariable: Entry.StoryVariable in order.
    Variables(Vec<(String, String, i32)>),
    Effect(Effect),
}

/// Why a request produced no NpcTalkMessage.
#[derive(Debug, PartialEq, Eq)]
pub enum TalkError {
    /// A source path that never calls its completion: the query stays open.
    Halted,
    /// A branch whose service is not migrated; nothing was sent or changed.
    Unsupported(String),
}

/// StoryEntity query fields kept between client messages.
#[derive(Clone, Debug)]
pub struct Session {
    result: Option<Vec<TalkEntity>>,
    pub building: String,
    pub querying_npc: String,
    pub talking_npc: String,
    pub story_line: String,
    pub last_button_set: Option<NodeId>,
    pub last_server_command: Option<NodeId>,
    pub can_start_query: bool,
    can_process_server_command: bool,
    pub story_line_changed: bool,
    is_error_handling: bool,
}
impl Default for Session {
    fn default() -> Self {
        Self {
            result: None,
            building: String::new(),
            querying_npc: String::new(),
            talking_npc: String::new(),
            story_line: String::new(),
            last_button_set: None,
            last_server_command: None,
            can_start_query: true,
            can_process_server_command: false,
            story_line_changed: false,
            is_error_handling: false,
        }
    }
}

/// Everything one request produced, in order.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Pushed {
    pub outputs: Vec<Output>,
}
impl Pushed {
    /// The NpcTalk messages (PushQueryResult) in order.
    pub fn messages(&self) -> Vec<&Vec<TalkEntity>> {
        self.outputs
            .iter()
            .filter_map(|o| match o {
                Output::Talk(t) => Some(t),
                _ => None,
            })
            .collect()
    }
}

enum K {
    Push,
    DefaultPush,
    Chain(NodeId, Box<K>),
    ClearSave(Box<K>),
    SaveThen(Option<String>, NodeId, Box<K>),
}

pub struct Talker<'a> {
    pub scripts: &'a NpcScripts,
    pub policy: &'a FeaturePolicy,
    pub facts: &'a dyn Facts,
    pub state: &'a mut StoryState,
    /// System.Random.Next(n) of a fresh Random.
    pub pick: &'a mut dyn FnMut(usize) -> usize,
    /// DateTime.UtcNow.Minute * 60 + Second.
    pub minute_second: i32,
    pub session: &'a mut Session,
    pushed: Pushed,
    depth: usize,
}

const MAX_DEPTH: usize = 20_000;

impl<'a> Talker<'a> {
    pub fn new(
        scripts: &'a NpcScripts,
        policy: &'a FeaturePolicy,
        facts: &'a dyn Facts,
        state: &'a mut StoryState,
        pick: &'a mut dyn FnMut(usize) -> usize,
        minute_second: i32,
        session: &'a mut Session,
    ) -> Self {
        Self {
            scripts,
            policy,
            facts,
            state,
            pick,
            minute_second,
            session,
            pushed: Pushed::default(),
            depth: 0,
        }
    }
    fn add(&mut self, e: TalkEntity) {
        if let Some(r) = self.session.result.as_mut() {
            r.push(e);
        }
    }
    fn node(&self, id: NodeId) -> &Node {
        &self.scripts.arena.nodes[id]
    }
    fn next(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id) {
            Node::Entity { next, .. }
            | Node::Do { next, .. }
            | Node::DoRandom { next, .. }
            | Node::CheckTime { next, .. }
            | Node::ButtonSet { next, .. }
            | Node::Button { next, .. }
            | Node::Timer { next, .. } => *next,
            Node::If { .. } | Node::Switch { .. } => None,
        }
    }
    fn is_server(&self, id: NodeId) -> bool {
        matches!(self.node(id), Node::Entity { command, .. } if SERVER_COMMANDS.contains(&command.as_str()))
    }
    fn command_of(&self, id: NodeId) -> Option<&str> {
        match self.node(id) {
            Node::Entity { command, .. } => Some(command),
            _ => None,
        }
    }
    fn enter(&mut self) -> Result<(), TalkError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(TalkError::Unsupported("script depth".into()));
        }
        Ok(())
    }

    /// INpcCommandExternal.DoChain.
    fn do_chain(&mut self, start: NodeId, k: K) -> Result<(), TalkError> {
        self.enter()?;
        let server = self.is_server(start);
        if server && !self.session.can_process_server_command {
            let mut walk = start;
            while let Some(n) = self.next(walk).filter(|&n| self.is_server(n)) {
                walk = n;
            }
            let n = self.next(walk);
            let last = (n.is_none() && self.command_of(walk) != Some("talk_to"))
                || n.is_some_and(|n| {
                    matches!(self.node(n), Node::ButtonSet { .. })
                        || self.command_of(n) == Some("button_set")
                });
            self.add(entity(
                "has_server_command",
                Some(if last { "last" } else { "" }),
                None,
            ));
            self.session.last_server_command = Some(start);
            self.push();
            return Ok(());
        }
        if !server {
            self.session.can_process_server_command = false;
            return match self.next(start) {
                None => {
                    let clears = match self.node(start) {
                        Node::Entity { .. } => true,
                        Node::Button { set, .. } => !matches!(
                            self.node(*set),
                            Node::ButtonSet {
                                force_select: true,
                                ..
                            }
                        ),
                        _ => false,
                    };
                    if clears {
                        self.clear_save_point();
                    }
                    self.process(start, k)
                }
                Some(n) => self.process(start, K::Chain(n, Box::new(k))),
            };
        }
        match self.next(start) {
            None => self.process(start, K::ClearSave(Box::new(k))),
            Some(n) => {
                let id = match self.node(start) {
                    Node::Entity { id, .. } => id.clone(),
                    _ => None,
                };
                self.process(start, K::SaveThen(id, n, Box::new(k)))
            }
        }
    }
    fn resume(&mut self, k: K) -> Result<(), TalkError> {
        match k {
            K::Push => {
                self.push();
                Ok(())
            }
            K::DefaultPush => self.default_push(),
            K::Chain(n, k) => self.do_chain(n, *k),
            K::ClearSave(k) => {
                self.clear_save_point();
                self.resume(*k)
            }
            K::SaveThen(id, n, k) => {
                if let Some(id) = id {
                    self.set_story_save(&id);
                }
                self.do_chain(n, *k)
            }
        }
    }
    fn process(&mut self, id: NodeId, k: K) -> Result<(), TalkError> {
        self.enter()?;
        match self.node(id).clone() {
            Node::Entity {
                command,
                arg1,
                arg2,
                ..
            } => {
                if self.is_server(id) {
                    self.server_command(&command, arg1.as_deref(), arg2.as_deref(), k)
                } else {
                    self.client_command(&command, arg1.as_deref(), arg2.as_deref(), k)
                }
            }
            Node::Do { commands, .. } => match commands.first() {
                None => self.resume(k),
                Some(&first) => self.do_chain(first, k),
            },
            Node::DoRandom { commands, .. } => {
                if commands.is_empty() {
                    return self.resume(k);
                }
                let choice = (self.pick)(commands.len()).min(commands.len() - 1);
                self.do_chain(commands[choice], k)
            }
            Node::If {
                condition,
                then,
                otherwise,
                fail,
            } => match (self.condition(condition)?, fail) {
                (None, Some(f)) => self.process(f, k),
                (Some(true), _) => self.process(then, k),
                _ => self.process(otherwise, k),
            },
            Node::Switch { branches } => {
                let class = usize::try_from(self.facts.class()).ok().filter(|c| *c < 10);
                let branch = class
                    .and_then(|c| branches[c])
                    .or(branches[SWITCH_BRANCHES.len() - 1]);
                match branch {
                    Some(b) => self.process(b, k),
                    None => Err(TalkError::Halted),
                }
            }
            Node::CheckTime {
                condition,
                then,
                next,
            } => {
                if self.condition(condition)? == Some(true) {
                    self.process(then, k)
                } else {
                    // next.Process with a null next throws in the source.
                    let n = next.ok_or(TalkError::Halted)?;
                    self.process(n, k)
                }
            }
            Node::ButtonSet { .. } => {
                self.add(entity("clear_button", Some(""), Some("")));
                self.session.last_button_set = Some(id);
                self.resume(k)
            }
            Node::Button {
                set,
                caption,
                command,
                condition,
                action,
                next,
            } => {
                let shown = match condition {
                    None => true,
                    Some(c) => self.condition(c)? == Some(true),
                };
                if shown {
                    let arg2 = if action.is_none() {
                        command.as_deref()
                    } else {
                        Some("")
                    };
                    self.add(entity("add_button", Some(&caption), arg2));
                }
                let force = matches!(
                    self.node(set),
                    Node::ButtonSet {
                        force_select: true,
                        ..
                    }
                );
                if next.is_none() && !force {
                    self.add(entity("finish_talk", None, None));
                }
                self.resume(k)
            }
            Node::Timer {
                telepathy,
                frequency,
                condition,
                ..
            } => {
                let left = if frequency > 0 {
                    frequency - self.minute_second % frequency
                } else {
                    0
                };
                if let Some(c) = condition {
                    // The result is ignored; the timer is always added.
                    self.condition(c)?;
                }
                self.add(entity(
                    "add_timer",
                    Some(&telepathy),
                    Some(&left.to_string()),
                ));
                self.resume(k)
            }
        }
    }
    /// ProcessCommandFunc.Process_* that are migrated.
    fn server_command(
        &mut self,
        command: &str,
        arg1: Option<&str>,
        arg2: Option<&str>,
        k: K,
    ) -> Result<(), TalkError> {
        match command {
            "ask_repair" => {
                let price = self.facts.repair_price().to_string();
                self.add(entity("talk_repair", arg1, Some(&price)));
                self.resume(k)
            }
            "do_repair" => {
                if self.facts.repair_price() <= self.facts.gold() {
                    self.pushed.outputs.push(Output::Effect(Effect::RepairAll));
                    self.add(entity("do_repair_successful", None, None));
                    self.resume(k)
                } else {
                    self.on_error("do_repair")
                }
            }
            other => self.story_command(other, arg1, arg2, k),
        }
    }
    /// int.Parse (leading/trailing white space allowed).
    fn parse(value: Option<&str>) -> Option<i32> {
        value.and_then(|v| v.trim().parse::<i32>().ok())
    }
    fn item_count(&self, class: &str) -> i64 {
        i64::from(self.facts.item_count(class))
            + self
                .state
                .item_delta
                .iter()
                .filter(|(c, _)| c == class)
                .map(|(_, d)| *d)
                .sum::<i64>()
    }
    fn add_delta(&mut self, class: &str, delta: i64) {
        match self.state.item_delta.iter_mut().find(|(c, _)| c == class) {
            Some(e) => e.1 += delta,
            None => self.state.item_delta.push((class.to_owned(), delta)),
        }
    }
    fn querying_line(&self, id: Option<&str>) -> String {
        match id {
            Some(v) if !v.is_empty() => v.to_owned(),
            _ => self.session.story_line.clone(),
        }
    }
    /// StoryEntity.SetVariable followed by SendStoryVariablesMessage.
    fn set_variable(&mut self, key: &str, value: i32) -> Result<(), TalkError> {
        let line = self.session.story_line.clone();
        let index = self
            .state
            .variables
            .iter()
            .position(|(l, k, _)| *l == line && k == key);
        match (index, value) {
            (None, 0) => return Err(TalkError::Halted),
            (None, v) => self.state.variables.push((line, key.to_owned(), v)),
            (Some(i), 0) => {
                self.state.variables.remove(i);
            }
            (Some(i), v) => self.state.variables[i].2 = v,
        }
        let variables = self.state.variables.clone();
        self.pushed.outputs.push(Output::Variables(variables));
        Ok(())
    }
    /// Process_* commands of the story service.
    fn story_command(
        &mut self,
        command: &str,
        arg1: Option<&str>,
        arg2: Option<&str>,
        k: K,
    ) -> Result<(), TalkError> {
        match command {
            "set_guide" => {
                let guide = (arg1.unwrap_or("").to_owned(), arg2.unwrap_or("").to_owned());
                self.state.guide = Some(guide.clone());
                // SetGuide → SaveGuide → SyncGuideInfo, then the command syncs again.
                self.pushed
                    .outputs
                    .push(Output::Guide(guide.0.clone(), guide.1.clone()));
                self.pushed.outputs.push(Output::Guide(guide.0, guide.1));
                self.resume(k)
            }
            "clear_guide" => {
                // ClearGuide dereferences Entry.StoryGuide.
                if self.state.guide.is_none() {
                    return Err(TalkError::Halted);
                }
                self.state.guide = Some((String::new(), String::new()));
                self.pushed
                    .outputs
                    .push(Output::Guide(String::new(), String::new()));
                self.pushed
                    .outputs
                    .push(Output::Guide(String::new(), String::new()));
                self.resume(k)
            }
            "reveal_storyline" => {
                let line = self.querying_line(arg1);
                let index = self
                    .state
                    .progress
                    .iter()
                    .position(|p| p.story_line == line);
                if let Some(i) = index
                    && self.state.progress[i].status == 255
                {
                    // RevealStoryLine: SetStoryLineStatus(Normal).
                    self.state.progress[i].status = 0;
                    let enabled = match self.scripts.story_line(&line) {
                        Some(l) => NpcScripts::is_story_enabled(&l.info, self.policy)
                            .map_err(|_| TalkError::Unsupported("feature".into()))?,
                        None => false,
                    };
                    if enabled {
                        self.session.story_line_changed = true;
                        self.add(entity("reveal_storyline_successful", Some(&line), None));
                    }
                }
                match index {
                    Some(i) if self.state.progress[i].status != 255 => self.resume(k),
                    _ => self.on_error("reveal_storyline"),
                }
            }
            "goto_phase" => {
                let line = self.session.story_line.clone();
                let target = arg1.unwrap_or("");
                let Some(phase) = self.scripts.story_line(&line).and_then(|l| l.phase(target))
                else {
                    let text =
                        format!("Error : goto_phase : 페이즈가 존재하지 않습니다.({target})");
                    self.add(entity("talk", Some(&text), None));
                    return self.resume(k);
                };
                let on_enter = phase.on_enter;
                let Some(p) = self
                    .state
                    .progress
                    .iter_mut()
                    .find(|p| p.story_line == line)
                else {
                    return self.on_error("goto_phase");
                };
                p.phase = Some(target.to_owned());
                self.session.story_line_changed = true;
                match on_enter {
                    Some(enter) => self.process(enter, k),
                    None => self.resume(k),
                }
            }
            "start_storyline" => {
                let id = arg1.unwrap_or("").to_owned();
                let Some(line) = self.scripts.story_line(&id) else {
                    let text =
                        format!("Error : start_storyline : 스토리라인이 존재하지 않습니다.({id})");
                    self.add(entity("talk", Some(&text), None));
                    return self.resume(k);
                };
                if !line.info.is_allowed_to(self.facts.class()) {
                    return self.on_error("start_storyline");
                }
                if self.state.progress.iter().any(|p| p.story_line == id) {
                    // AlreadyOwned / AlreadyCompleted.
                    return self.resume(k);
                }
                let opened = self
                    .scripts
                    .open_story_line(line)
                    .map_err(|_| TalkError::Halted)?;
                self.state.progress.push(opened);
                if NpcScripts::is_story_enabled(&line.info, self.policy)
                    .map_err(|_| TalkError::Unsupported("feature".into()))?
                {
                    self.session.story_line_changed = true;
                    if line.info.auto_reveal {
                        self.add(entity("reveal_storyline_successful", Some(&id), None));
                    }
                }
                self.resume(k)
            }
            "complete_storyline" => {
                let line_id = self.querying_line(arg1);
                let index = self
                    .state
                    .progress
                    .iter()
                    .position(|p| p.story_line == line_id && p.phase.is_some());
                if let Some(i) = index {
                    if self.state.progress[i].status != 255 {
                        self.add(entity(
                            "complete_storyline_successful",
                            Some(&line_id),
                            None,
                        ));
                    }
                    let info = self
                        .scripts
                        .story_line(&line_id)
                        .ok_or(TalkError::Halted)?
                        .info
                        .clone();
                    // EXP/AP (level up), titles, skills and item rewards are not migrated.
                    if info.reward_exp != 0
                        || info.reward_ap != 0
                        || info.reward_title != 0
                        || info.reward_skill.is_some()
                        || info.reward_item.is_some()
                    {
                        return Err(TalkError::Unsupported("story reward".into()));
                    }
                    self.state.progress[i].phase = None;
                    self.state.variables.retain(|(l, _, _)| *l != line_id);
                    if info.reward_gold != 0 {
                        let rate = self
                            .policy
                            .get_integer("GoldRate_Story")
                            .map_err(|_| TalkError::Unsupported("GoldRate_Story".into()))?;
                        let gold = info.reward_gold.wrapping_mul(rate) / 100;
                        self.pushed.outputs.push(Output::Effect(Effect::GiveItem {
                            class: "gold".to_owned(),
                            count: gold,
                        }));
                        self.add(entity(
                            "give_item_successful",
                            Some("gold"),
                            Some(&gold.to_string()),
                        ));
                    }
                    self.session.story_line_changed = true;
                }
                let done = self
                    .state
                    .progress
                    .iter()
                    .any(|p| p.story_line == line_id && p.phase.is_none());
                if done {
                    self.resume(k)
                } else {
                    self.on_error("complete_storyline")
                }
            }
            "talk_to" => {
                let target = arg1.unwrap_or("");
                let parts: Vec<&str> = target.split(['/', '\\']).collect();
                let (building, npc) = match parts[..] {
                    [b, n] => (Some(b.to_owned()), n.to_owned()),
                    _ => (None, target.to_owned()),
                };
                let line = self.querying_line(arg2);
                if self.session.story_line_changed {
                    self.session.story_line_changed = false;
                    self.pushed
                        .outputs
                        .push(Output::SyncStory(self.state.progress.clone()));
                }
                self.clear_save_point();
                // TalkTo starts a new query; this command's completion is dropped.
                let _ = k;
                self.talk_to_inner(building.as_deref(), &npc, Some(&line), Some(""))
            }
            "give_item" => {
                let class = arg1.unwrap_or("");
                let Some(count) = Self::parse(arg2) else {
                    let text = format!(
                        "Error : give_item에 Arg2가 숫자가 아닙니다.({})",
                        arg2.unwrap_or("")
                    );
                    self.add(entity("talk", Some(&text), None));
                    return self.resume(k);
                };
                if class.contains('[') || class.contains("_lvl") {
                    return Err(TalkError::Unsupported("give_item ItemClassEx".into()));
                }
                self.pushed.outputs.push(Output::Effect(Effect::GiveItem {
                    class: class.to_owned(),
                    count,
                }));
                self.add_delta(class, i64::from(count));
                self.add(entity("give_item_successful", Some(class), arg2));
                self.resume(k)
            }
            "take_item" => {
                let class = arg1.unwrap_or("");
                let Some(count) = Self::parse(arg2) else {
                    let text = format!(
                        "Error : take_item에 Arg2가 숫자가 아닙니다.({})",
                        arg2.unwrap_or("")
                    );
                    self.add(entity("talk", Some(&text), None));
                    return self.resume(k);
                };
                if count >= 0 && self.item_count(class) >= i64::from(count) {
                    self.pushed.outputs.push(Output::Effect(Effect::TakeItem {
                        class: class.to_owned(),
                        count,
                    }));
                    self.add_delta(class, -i64::from(count));
                    self.add(entity("take_item_successful", Some(class), arg2));
                    self.resume(k)
                } else {
                    self.on_error("take_item")
                }
            }
            "set_token" | "increase_token" | "decrease_token" => {
                let key = arg1.unwrap_or("");
                let line = self.session.story_line.clone();
                let amount = match Self::parse(arg2) {
                    Some(v) => v,
                    None => self.state.variable(&line, arg2.unwrap_or("")),
                };
                let current = self.state.variable(&line, key);
                let value = match command {
                    "set_token" => amount,
                    "increase_token" => current.wrapping_add(amount),
                    _ => current.wrapping_sub(amount),
                };
                self.set_variable(key, value)?;
                self.resume(k)
            }
            other => Err(TalkError::Unsupported(other.to_owned())),
        }
    }
    /// StoryEntity.SetStorySave(cmdID) for the querying NPC and storyline.
    fn set_story_save(&mut self, command: &str) {
        let (npc, line) = (
            self.session.querying_npc.clone(),
            self.session.story_line.clone(),
        );
        match self
            .state
            .saves
            .iter_mut()
            .find(|(n, l, _)| *n == npc && *l == line)
        {
            Some(save) => save.2 = command.to_owned(),
            None => self.state.saves.push((npc, line, command.to_owned())),
        }
    }
    /// StoryEntity.ClearSavePoint (skipped while handling an error).
    fn clear_save_point(&mut self) {
        if self.session.is_error_handling {
            return;
        }
        let (npc, line) = (&self.session.querying_npc, &self.session.story_line);
        self.state.saves.retain(|(n, l, _)| n != npc || l != line);
    }
    /// StoryEntity.OnError: the NPC's "<cmd>_fail" talk, or the null-error entity.
    fn on_error(&mut self, command: &str) -> Result<(), TalkError> {
        self.session.is_error_handling = true;
        let npc = self.session.talking_npc.clone();
        let building = self.scripts.base.default_building(&npc);
        let fail = format!("{command}_fail");
        match self.scripts.base.command_talk(&building, &npc, &fail) {
            Some(talk) => {
                let content = talk.content;
                self.process(content, K::DefaultPush)
            }
            None => {
                let text = format!("GameUI_Heroes_NPCTalkEntity_Talk_NullError,{command}");
                self.add(entity("Error", Some(&text), None));
                self.default_push()
            }
        }
    }
    /// ProcessCommandFunc.ProcessClientCmd.
    fn client_command(
        &mut self,
        command: &str,
        arg1: Option<&str>,
        arg2: Option<&str>,
        k: K,
    ) -> Result<(), TalkError> {
        match command {
            "telepathy_goal" => {
                let goal = self
                    .state
                    .progress
                    .iter()
                    .find(|p| p.story_line == self.session.story_line)
                    .and_then(|p| {
                        let line = self.scripts.story_line(&p.story_line)?;
                        line.phase(p.phase.as_deref()?).map(|ph| ph.goal.clone())
                    });
                match goal {
                    Some(g) => self.add(entity("telepathy", Some(&g), None)),
                    None => self.add(entity("talk", Some("no goal for telepathy_goal"), None)),
                }
                self.resume(k)
            }
            "button_set" => {
                let set = arg1.and_then(|a| self.scripts.base.button_sets.get(a).copied());
                match set {
                    Some(s) => self.do_chain(s, k),
                    None => {
                        let message = format!("버튼셋 {}이 없습니다.", arg1.unwrap_or(""));
                        self.add(entity("Error", Some(&message), None));
                        let default = self
                            .scripts
                            .base
                            .default_button_set
                            .ok_or(TalkError::Halted)?;
                        self.do_chain(default, k)
                    }
                }
            }
            "change_npc" => {
                self.session.talking_npc = arg1.unwrap_or("").to_owned();
                self.add(entity(command, arg1, arg2));
                self.resume(k)
            }
            "timer" => {
                let frequency = arg1.and_then(|a| a.trim().parse::<i32>().ok()).unwrap_or(0);
                let left = if frequency > 0 {
                    frequency - self.minute_second % frequency
                } else {
                    0
                };
                self.add(entity("timer", Some(&left.to_string()), arg2));
                self.resume(k)
            }
            // "talk" and every other client command are forwarded unchanged.
            _ => {
                self.add(entity(command, arg1, arg2));
                self.resume(k)
            }
        }
    }
    /// And/Or/entity Query with the `Next` chaining of their parent list.
    fn condition(&mut self, id: usize) -> Result<Option<bool>, TalkError> {
        self.enter()?;
        match self.scripts.arena.conds[id].clone() {
            Cond::And { conditions, .. } => match conditions.first() {
                None => Ok(Some(true)),
                Some(&first) => self.chain(first, true),
            },
            Cond::Or { conditions, .. } => match conditions.first() {
                None => Ok(Some(true)),
                Some(&first) => self.chain(first, false),
            },
            Cond::Entity {
                command,
                arg1,
                arg2,
                ..
            } => self.entity_condition(&command, arg1.as_deref(), arg2.as_deref()),
        }
    }
    /// And.DoChain continues while true; Or.DoChain while false.
    fn chain(&mut self, start: usize, and: bool) -> Result<Option<bool>, TalkError> {
        let next = match &self.scripts.arena.conds[start] {
            Cond::Entity { next, .. } | Cond::And { next, .. } | Cond::Or { next, .. } => *next,
        };
        let result = self.condition(start)?;
        match next {
            Some(n) if result == Some(and) => self.chain(n, and),
            _ => Ok(result),
        }
    }
    fn entity_condition(
        &mut self,
        command: &str,
        arg1: Option<&str>,
        arg2: Option<&str>,
    ) -> Result<Option<bool>, TalkError> {
        if let Some(rest) = command.strip_prefix("not_") {
            if !CONDITIONS.contains(&rest) {
                return Ok(None);
            }
            return Ok(self.evaluate(rest, arg1, arg2)?.map(|r| !r));
        }
        if !CONDITIONS.contains(&command) {
            return Ok(None);
        }
        self.evaluate(command, arg1, arg2)
    }
    fn evaluate(
        &mut self,
        command: &str,
        arg1: Option<&str>,
        arg2: Option<&str>,
    ) -> Result<Option<bool>, TalkError> {
        let number = |s: Option<&str>| s.and_then(|v| v.trim().parse::<i32>().ok());
        // `int x = -1; int.TryParse(s, out x)`: a failed parse stores 0, only "-1" is null.
        let level = |s: Option<&str>| Some(number(s).unwrap_or(0)).filter(|v| *v != -1);
        let line = self.session.story_line.clone();
        let progress = |id: Option<&str>| {
            self.state
                .progress
                .iter()
                .find(|p| Some(p.story_line.as_str()) == id)
        };
        Ok(match command {
            "class" => Some(
                CLASS_NAMES
                    .get(self.facts.class() as usize)
                    .is_some_and(|n| arg1.is_some_and(|a| n.eq_ignore_ascii_case(a))),
            ),
            "level" => level(arg1).map(|l| self.facts.level() == l),
            "level_max" => level(arg1).map(|l| self.facts.level() <= l),
            "level_min" => level(arg1).map(|l| self.facts.level() >= l),
            "has_title" => match number(arg1) {
                // int.TryParse failure returns without completing the query.
                None => return Err(TalkError::Halted),
                Some(id) => Some(self.facts.title_acquired(id)),
            },
            "has_item" => level(arg2).map(|c| self.item_count(arg1.unwrap_or("")) >= i64::from(c)),
            // Items carry no ENHANCE attribute in the migrated inventory: EnhanceLevel 0.
            "has_item_enhanced" => {
                level(arg2).map(|l| self.item_count(arg1.unwrap_or("")) > 0 && 0 >= l)
            }
            "has_quest" => Some(self.facts.quest_progress(arg1.unwrap_or("")) >= 0),
            "quest_cleared" => Some(self.facts.quest_progress(arg1.unwrap_or("")) > 0),
            "has_storyline" => Some(progress(arg1).is_some()),
            "storyline_cleared" => Some(progress(arg1).is_some_and(|p| p.phase.is_none())),
            "storyline_is" => Some(progress(arg1).is_some_and(|p| p.phase.as_deref() == arg2)),
            "feature" => Some(
                self.policy
                    .is_enabled(arg1)
                    .map_err(|_| TalkError::Unsupported("feature".into()))?,
            ),
            "has_manufacture" => Some(self.facts.manufacture(arg1.unwrap_or("")).is_some()),
            "is_manufacture_level" => match number(arg2) {
                None => None,
                Some(l) => self
                    .facts
                    .manufacture(arg1.unwrap_or(""))
                    .map(|(g, _)| g == l),
            },
            "is_manufacture_reachexp" => match number(arg2) {
                None => None,
                Some(e) => self
                    .facts
                    .manufacture(arg1.unwrap_or(""))
                    .map(|(_, x)| x >= e),
            },
            "token_min" => {
                number(arg2).map(|v| self.state.variable(&line, arg1.unwrap_or("")) >= v)
            }
            "token_max" => {
                number(arg2).map(|v| self.state.variable(&line, arg1.unwrap_or("")) <= v)
            }
            "token_is" => number(arg2).map(|v| self.state.variable(&line, arg1.unwrap_or("")) == v),
            // A timer variable is only written by set_timer (not migrated): 0 is null.
            "time_passed" => match number(arg2) {
                None => None,
                Some(_) if self.state.variable(&line, arg1.unwrap_or("")) == 0 => None,
                Some(_) => return Err(TalkError::Unsupported("time_passed".into())),
            },
            other => return Err(TalkError::Unsupported(other.to_owned())),
        })
    }

    fn push(&mut self) {
        if let Some(r) = self.session.result.take() {
            self.pushed.outputs.push(Output::Talk(r));
            if self.session.story_line_changed {
                self.session.story_line_changed = false;
                self.pushed
                    .outputs
                    .push(Output::SyncStory(self.state.progress.clone()));
            }
        }
        self.session.can_start_query = true;
    }
    fn default_push(&mut self) -> Result<(), TalkError> {
        if self.session.last_button_set.is_none() {
            let default = self
                .scripts
                .base
                .default_button_set
                .ok_or(TalkError::Halted)?;
            self.do_chain(default, K::Push)
        } else {
            self.push();
            Ok(())
        }
    }
    fn start_query(&mut self, building: &str, npc: &str, story_line: &str) {
        let s = &mut *self.session;
        s.result = Some(vec![entity("change_npc", Some(npc), None)]);
        s.building = building.to_owned();
        s.querying_npc = npc.to_owned();
        s.talking_npc = npc.to_owned();
        s.story_line = story_line.to_owned();
        s.last_button_set = None;
        s.last_server_command = None;
        s.can_start_query = false;
        s.can_process_server_command = true;
        s.story_line_changed = false;
        s.is_error_handling = false;
    }
    fn resume_query(&mut self) {
        let s = &mut *self.session;
        if s.result.is_none() {
            s.result = Some(Vec::new());
        }
        s.can_start_query = false;
        s.can_process_server_command = true;
        s.story_line_changed = false;
    }
    fn talk_to_command(
        &mut self,
        building: &str,
        npc: &str,
        command: &str,
    ) -> Result<(), TalkError> {
        match self.scripts.base.command_talk(building, npc, command) {
            Some(talk) => {
                let content = talk.content;
                self.process(content, K::DefaultPush)
            }
            None => {
                self.add(entity(
                    "Error",
                    Some(&format!("No Talk For {npc}/{command}")),
                    None,
                ));
                self.default_push()
            }
        }
    }
    fn talk_to_story_line(
        &mut self,
        building: &str,
        npc: &str,
        line_id: &str,
    ) -> Result<(), TalkError> {
        let Some(line) = self.scripts.story_line(line_id) else {
            self.add(entity(
                "talk",
                Some(&format!("no storyline!!! : {line_id}")),
                None,
            ));
            return self.default_push();
        };
        if !NpcScripts::is_story_enabled(&line.info, self.policy)
            .map_err(|_| TalkError::Unsupported("feature".into()))?
        {
            if line.phase("FeatureOff").is_none() {
                self.add(entity(
                    "talk",
                    Some(&format!("invalid storyline!!! : {line_id}")),
                    None,
                ));
                return self.default_push();
            }
            return Err(TalkError::Unsupported("GotoPhase".into()));
        }
        // NpcManager.GetStoryLineTalk: default building and NPC must exist; the last
        // phase talk added for the storyline wins.
        let building_known = self
            .scripts
            .base
            .default_npcs
            .iter()
            .any(|(b, _)| b == building);
        let mut found = None;
        if building_known {
            for p in &self.state.progress {
                let Some(l) = self.scripts.story_line(&p.story_line) else {
                    continue;
                };
                let Some(phase) = p.phase.as_deref().and_then(|ph| l.phase(ph)) else {
                    continue;
                };
                for t in &phase.talks {
                    if t.key.0 == building && t.key.1 == npc && p.story_line == line_id {
                        found = Some(t.content);
                    }
                }
            }
        }
        match found {
            Some(content) => self.process(content, K::DefaultPush),
            None => {
                self.add(entity(
                    "talk",
                    Some(&format!(
                        "no talk for this NPC!!! : {line_id} :{building}/{npc}"
                    )),
                    None,
                ));
                self.default_push()
            }
        }
    }

    /// StoryEntity.TalkTo (QueryNpcTalkProcessor after the CanStartQuery/DEV checks).
    pub fn talk_to(
        mut self,
        building: Option<&str>,
        npc: &str,
        story_line: Option<&str>,
        command: Option<&str>,
    ) -> Result<Pushed, TalkError> {
        self.talk_to_inner(building, npc, story_line, command)?;
        Ok(self.pushed)
    }
    /// StoryEntity.TalkTo_SavePoint: resume after the saved server command.
    fn talk_to_save_point(&mut self, line: &str, command: &str) -> Result<bool, TalkError> {
        let saved = if line == "default" {
            self.scripts.base.server_commands.get(command).copied()
        } else {
            match self.scripts.story_line(line) {
                Some(l) => l.server_commands.get(command).copied(),
                None => {
                    self.clear_save_point();
                    return Ok(false);
                }
            }
        };
        let resume = saved.and_then(|c| self.next(c).map(|n| (c, n)));
        let Some((saved, next)) = resume else {
            self.clear_save_point();
            return Ok(false);
        };
        self.session.story_line = line.to_owned();
        if let Node::Entity {
            talking_npc: Some(npc),
            ..
        } = self.node(saved)
        {
            self.session.result = Some(vec![entity("change_npc", Some(npc), None)]);
        }
        self.do_chain(next, K::DefaultPush)?;
        Ok(true)
    }
    fn talk_to_inner(
        &mut self,
        building: Option<&str>,
        npc: &str,
        story_line: Option<&str>,
        command: Option<&str>,
    ) -> Result<(), TalkError> {
        let building = match building {
            Some(b) if !b.is_empty() => b.to_owned(),
            _ => self.scripts.base.default_building(npc),
        };
        let story_line = match story_line {
            Some(s) if !s.is_empty() => s.to_owned(),
            _ => "default".to_owned(),
        };
        self.start_query(&building, npc, &story_line);
        let feature = self.scripts.base.feature(npc).to_owned();
        if !feature.is_empty()
            && !self
                .policy
                .is_enabled(Some(&feature))
                .map_err(|_| TalkError::Unsupported("feature".into()))?
        {
            self.add(entity(
                "talk",
                Some("GameUI_Heroes_NPCTalkEntity_Talk_Feature_Disable"),
                None,
            ));
            return self.default_push();
        }
        let save = self
            .state
            .saves
            .iter()
            .find(|(n, l, _)| n == npc && *l == story_line)
            .map(|s| s.2.clone());
        if let Some(command) = save
            && self.talk_to_save_point(&story_line, &command)?
        {
            return Ok(());
        }
        if command == Some("story_sector") {
            return Err(TalkError::Unsupported("story_sector".into()));
        }
        if story_line != "default" {
            self.talk_to_story_line(&building, npc, &story_line)
        } else {
            self.talk_to_command(&building, npc, command.unwrap_or(""))
        }
    }
    /// StoryEntity.SelectButton.
    pub fn select_button(mut self, index: i32) -> Result<Pushed, TalkError> {
        self.resume_query();
        let Some(set) = self.session.last_button_set else {
            self.default_push()?;
            return Ok(self.pushed);
        };
        let Node::ButtonSet {
            id, timer, buttons, ..
        } = self.node(set).clone()
        else {
            return Err(TalkError::Halted);
        };
        if usize::try_from(index).is_ok_and(|i| buttons.len() <= i) {
            let message = format!(
                "Invalid ButtonSetInfo ( {} / {index} )",
                id.unwrap_or_default()
            );
            self.add(entity("Error", Some(&message), None));
            self.default_push()?;
            return Ok(self.pushed);
        }
        if index < 0 {
            match timer {
                None => {
                    self.add(entity("Error", Some("no timer"), None));
                    self.default_push()?;
                }
                Some(t) => {
                    self.session.last_button_set = None;
                    let Node::Timer { action, .. } = self.node(t).clone() else {
                        return Err(TalkError::Halted);
                    };
                    self.do_chain(action, K::DefaultPush)?;
                }
            }
            return Ok(self.pushed);
        }
        let button = buttons[index as usize];
        let Node::Button {
            command, action, ..
        } = self.node(button).clone()
        else {
            return Err(TalkError::Halted);
        };
        match action {
            None => {
                if let Some(c) = command {
                    let (b, n) = (
                        self.session.building.clone(),
                        self.session.talking_npc.clone(),
                    );
                    self.talk_to_command(&b, &n, &c)?;
                }
                self.session.last_button_set = None;
            }
            Some(a) => {
                self.session.last_button_set = None;
                self.do_chain(a, K::DefaultPush)?;
            }
        }
        Ok(self.pushed)
    }
    /// StoryEntity.ExecuteServerCommand.
    pub fn execute_server_command(mut self) -> Result<Pushed, TalkError> {
        self.resume_query();
        match self.session.last_server_command.take() {
            None => {
                self.add(entity("Error", Some("No ServerCommand"), None));
                self.default_push()?;
            }
            Some(start) => self.do_chain(start, K::DefaultPush)?,
        }
        Ok(self.pushed)
    }
}

/// BaseCharacter names for Cond_class (ToString, ordinal ignore case).
const CLASS_NAMES: [&str; 10] = [
    "Lethita", "Fiona", "Evy", "Kalok", "Kay", "Vella", "Hurk", "Lynn", "Arisha", "Hagie",
];
/// ProcessConditionFunc.Cond_* names.
const CONDITIONS: [&str; 46] = [
    "class",
    "level",
    "level_max",
    "level_min",
    "has_title",
    "know_title",
    "has_skill",
    "skill_rank_min",
    "skill_rank_max",
    "skill_final_rank_min",
    "skill_final_rank_max",
    "has_item",
    "has_item_regex",
    "has_item_enhanced",
    "has_item_even_expired",
    "has_item_regex_even_expired",
    "has_quest",
    "quest_available",
    "quest_cleared",
    "goal_cleared",
    "quest_progress",
    "has_guild",
    "time_passed",
    "day_passed",
    "hour_within",
    "token_min",
    "token_max",
    "token_is",
    "date_within",
    "has_storyline",
    "storyline_cleared",
    "storyline_is",
    "feature",
    "has_manufacture",
    "is_manufacture_level",
    "is_manufacture_reachexp",
    "is_in_date_period",
    "is_account_secured",
    "is_cafe",
    "vocation_level_min",
    "vocation_level_max",
    "is_returned_character",
    "is_preset_character",
    "has_pet",
    "is_earlier",
    "is_later",
];
