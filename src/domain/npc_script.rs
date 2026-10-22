//! NPC script model recovered from PlayerService.StoryService: XmlLoader commands and
//! conditions, the INpcCommand `Next` setters and SetTalkingNpc, NpcBaseInfo (default
//! NPC talks, button sets, server command IDs) and StoryLine/Phase. Constructor
//! exceptions of the source become `ScriptError`; the source would refuse to load.
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("NPC script is outside the recovered loader rules: {0}")]
pub struct ScriptError(pub &'static str);

/// XElement after Load(LoadOptions.None): whitespace-only text nodes are dropped.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    pub name: String,
    pub attributes: Vec<(String, String)>,
    pub text: String,
    pub children: Vec<Element>,
    pub tail: String,
}
impl Element {
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    /// XElement.Element(name) / Elements(name).FirstOrDefault().
    pub fn first(&self, name: &str) -> Option<&Element> {
        self.children.iter().find(|c| c.name == name)
    }
    /// XElement.Value: text plus every child's Value and tail, in document order.
    pub fn value(&self) -> String {
        let mut out = self.text.clone();
        for c in &self.children {
            out.push_str(&c.value());
            out.push_str(&c.tail);
        }
        out
    }
}

pub type NodeId = usize;
pub type CondId = usize;

/// Character classes of Switch, in the source setter order.
pub const SWITCH_BRANCHES: [&str; 11] = [
    "Lethita", "Fiona", "Evy", "Kalok", "Kay", "Vella", "Hurk", "Lynn", "Arisha", "Hagie", "Fail",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    Entity {
        id: Option<String>,
        command: String,
        arg1: Option<String>,
        arg2: Option<String>,
        talking_npc: Option<String>,
        next: Option<NodeId>,
    },
    Do {
        commands: Vec<NodeId>,
        next: Option<NodeId>,
    },
    DoRandom {
        commands: Vec<NodeId>,
        next: Option<NodeId>,
    },
    If {
        condition: CondId,
        then: NodeId,
        otherwise: NodeId,
        fail: Option<NodeId>,
    },
    /// Branch Do blocks by SWITCH_BRANCHES index.
    Switch { branches: [Option<NodeId>; 11] },
    CheckTime {
        condition: CondId,
        then: NodeId,
        next: Option<NodeId>,
    },
    ButtonSet {
        id: Option<String>,
        timer: Option<NodeId>,
        buttons: Vec<NodeId>,
        force_select: bool,
        next: Option<NodeId>,
    },
    Button {
        set: NodeId,
        caption: String,
        command: Option<String>,
        condition: Option<CondId>,
        action: Option<NodeId>,
        next: Option<NodeId>,
    },
    Timer {
        set: NodeId,
        telepathy: String,
        frequency: i32,
        condition: Option<CondId>,
        action: NodeId,
        next: Option<NodeId>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cond {
    Entity {
        command: String,
        arg1: Option<String>,
        arg2: Option<String>,
        next: Option<CondId>,
    },
    And {
        conditions: Vec<CondId>,
        next: Option<CondId>,
    },
    Or {
        conditions: Vec<CondId>,
        next: Option<CondId>,
    },
}

/// Shared command/condition arena of every loaded script.
#[derive(Debug, Default)]
pub struct Arena {
    pub nodes: Vec<Node>,
    pub conds: Vec<Cond>,
}

const MAX_NODES: usize = 4_000_000;

fn push_node(arena: &mut Arena, node: Node) -> Result<NodeId, ScriptError> {
    if arena.nodes.len() >= MAX_NODES {
        return Err(ScriptError("node bound"));
    }
    arena.nodes.push(node);
    Ok(arena.nodes.len() - 1)
}
fn push_cond(arena: &mut Arena, cond: Cond) -> Result<CondId, ScriptError> {
    if arena.conds.len() >= MAX_NODES {
        return Err(ScriptError("condition bound"));
    }
    arena.conds.push(cond);
    Ok(arena.conds.len() - 1)
}

/// Source "\\n" escape inside attribute and talk text.
fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
}

impl Arena {
    /// XmlLoader.LoadCommand.
    pub fn load_command(&mut self, xml: &Element) -> Result<NodeId, ScriptError> {
        match xml.name.as_str() {
            "Do" => self.load_do(xml),
            "DoRandom" => {
                let commands = self.load_list(xml)?;
                push_node(
                    self,
                    Node::DoRandom {
                        commands,
                        next: None,
                    },
                )
            }
            "If" => self.load_if(xml),
            "Switch" => {
                let mut branches = [None; 11];
                for (slot, name) in branches.iter_mut().zip(SWITCH_BRANCHES) {
                    if let Some(e) = xml.first(name) {
                        *slot = Some(self.load_do(e)?);
                    }
                }
                push_node(self, Node::Switch { branches })
            }
            "ButtonSet" => self.load_button_set(xml),
            "CheckTime" => {
                let condition = self.load_and(
                    xml.first("Condition")
                        .ok_or(ScriptError("CheckTime without Condition"))?,
                )?;
                let then = self.load_do(
                    xml.first("Then")
                        .ok_or(ScriptError("CheckTime without Then"))?,
                )?;
                push_node(
                    self,
                    Node::CheckTime {
                        condition,
                        then,
                        next: None,
                    },
                )
            }
            _ => self.load_entity(xml),
        }
    }
    fn load_list(&mut self, xml: &Element) -> Result<Vec<NodeId>, ScriptError> {
        xml.children.iter().map(|c| self.load_command(c)).collect()
    }
    /// Do: children chained in order (`commands[i].Next = commands[i + 1]`).
    pub fn load_do(&mut self, xml: &Element) -> Result<NodeId, ScriptError> {
        let commands = self.load_list(xml)?;
        for pair in commands.windows(2) {
            self.set_next(pair[0], Some(pair[1]));
        }
        push_node(
            self,
            Node::Do {
                commands,
                next: None,
            },
        )
    }
    fn load_if(&mut self, xml: &Element) -> Result<NodeId, ScriptError> {
        let condition = self.load_and(
            xml.first("Condition")
                .ok_or(ScriptError("If without Condition"))?,
        )?;
        let then = self.load_do(xml.first("Then").ok_or(ScriptError("If without Then"))?)?;
        let otherwise = self.load_do(xml.first("Else").ok_or(ScriptError("If without Else"))?)?;
        let fail = xml.first("Fail").map(|f| self.load_do(f)).transpose()?;
        push_node(
            self,
            Node::If {
                condition,
                then,
                otherwise,
                fail,
            },
        )
    }
    fn load_entity(&mut self, xml: &Element) -> Result<NodeId, ScriptError> {
        let mut id = None;
        let mut arg1 = None;
        let mut arg2 = None;
        for (k, v) in &xml.attributes {
            match k.as_str() {
                "id" => id = Some(v.trim().to_owned()),
                "arg1" => arg1 = Some(unescape(v.trim())),
                "arg2" => arg2 = Some(unescape(v.trim())),
                _ => return Err(ScriptError("unknown command attribute")),
            }
        }
        let value = xml.value();
        if matches!(xml.name.as_str(), "talk" | "telepathy" | "exec_console") {
            arg1 = Some(unescape(&value));
        } else {
            let mut parts = value.split(',');
            if let (Some(first), None) = (parts.next(), arg1.as_ref()) {
                arg1 = Some(unescape(first.trim()));
            }
            if let (Some(second), None) = (parts.next(), arg2.as_ref()) {
                arg2 = Some(second.trim().to_owned());
            }
        }
        push_node(
            self,
            Node::Entity {
                id,
                command: xml.name.clone(),
                arg1,
                arg2,
                talking_npc: None,
                next: None,
            },
        )
    }
    fn load_button_set(&mut self, xml: &Element) -> Result<NodeId, ScriptError> {
        let set = push_node(
            self,
            Node::ButtonSet {
                id: xml.attribute("id").map(str::to_owned),
                timer: None,
                buttons: Vec::new(),
                force_select: true,
                next: None,
            },
        )?;
        let timer = match xml.first("Timer") {
            Some(t) => {
                let condition = t.first("Condition").map(|c| self.load_and(c)).transpose()?;
                let action = self
                    .load_action(t)?
                    .ok_or(ScriptError("Timer without action"))?;
                Some(push_node(
                    self,
                    Node::Timer {
                        set,
                        telepathy: t.attribute("telepathy").unwrap_or("").to_owned(),
                        // StringExtension.ToInt(0): int.TryParse or the default.
                        frequency: t
                            .attribute("frequency")
                            .and_then(|f| f.trim().parse().ok())
                            .unwrap_or(0),
                        condition,
                        action,
                        next: None,
                    },
                )?)
            }
            None => None,
        };
        let mut buttons = Vec::new();
        let mut force_select = true;
        for b in xml.children.iter().filter(|c| c.name == "Button") {
            let command = b.attribute("command").map(str::to_owned);
            let condition = b.first("Condition").map(|c| self.load_and(c)).transpose()?;
            let action = self.load_action(b)?;
            if command.is_none() && action.is_none() {
                return Err(ScriptError("Button without command and action"));
            }
            if matches!(command.as_deref(), Some("endtalk" | "endtalk_morvan")) {
                force_select = false;
            }
            let id = push_node(
                self,
                Node::Button {
                    set,
                    caption: b.attribute("caption").unwrap_or("no_caption").to_owned(),
                    command,
                    condition,
                    action,
                    next: None,
                },
            )?;
            if let Some(&previous) = buttons.last() {
                self.set_next(previous, Some(id));
            }
            buttons.push(id);
        }
        let first = buttons.first().copied();
        let next = match timer {
            Some(t) => {
                self.set_next(t, first);
                Some(t)
            }
            None => first,
        };
        self.nodes[set] = Node::ButtonSet {
            id: xml.attribute("id").map(str::to_owned),
            timer,
            buttons,
            force_select,
            next,
        };
        if let Some(npc) = xml.attribute("npc") {
            self.set_talking_npc(set, npc);
        }
        Ok(set)
    }
    /// Button/Timer action: the first Do, else DoRandom, else If.
    fn load_action(&mut self, xml: &Element) -> Result<Option<NodeId>, ScriptError> {
        if let Some(d) = xml.first("Do") {
            return self.load_do(d).map(Some);
        }
        if let Some(d) = xml.first("DoRandom") {
            return self.load_command(d).map(Some);
        }
        if let Some(i) = xml.first("If") {
            return self.load_if(i).map(Some);
        }
        Ok(None)
    }
    /// And(xml): conditions chained in order.
    pub fn load_and(&mut self, xml: &Element) -> Result<CondId, ScriptError> {
        let conditions = self.load_conditions(xml)?;
        push_cond(
            self,
            Cond::And {
                conditions,
                next: None,
            },
        )
    }
    fn load_conditions(&mut self, xml: &Element) -> Result<Vec<CondId>, ScriptError> {
        let conditions = xml
            .children
            .iter()
            .map(|c| self.load_condition(c))
            .collect::<Result<Vec<_>, _>>()?;
        for pair in conditions.windows(2) {
            match &mut self.conds[pair[0]] {
                Cond::Entity { next, .. } | Cond::And { next, .. } | Cond::Or { next, .. } => {
                    *next = Some(pair[1])
                }
            }
        }
        Ok(conditions)
    }
    fn load_condition(&mut self, xml: &Element) -> Result<CondId, ScriptError> {
        match xml.name.as_str() {
            "AND" => self.load_and(xml),
            "OR" => {
                let conditions = self.load_conditions(xml)?;
                push_cond(
                    self,
                    Cond::Or {
                        conditions,
                        next: None,
                    },
                )
            }
            // Not(xml) re-enters LoadCondition with the same element: unbounded recursion.
            "NOT" => Err(ScriptError("NOT recurses in the source loader")),
            _ => {
                let value = xml.value();
                let mut parts = value.split(',');
                let arg1 = parts.next().map(|p| p.trim().to_owned());
                let arg2 = parts.next().map(|p| p.trim().to_owned());
                push_cond(
                    self,
                    Cond::Entity {
                        command: xml.name.clone(),
                        arg1,
                        arg2,
                        next: None,
                    },
                )
            }
        }
    }
    /// INpcCommand.Next setters.
    pub fn set_next(&mut self, id: NodeId, value: Option<NodeId>) {
        match self.nodes[id].clone() {
            Node::Entity { .. } | Node::Button { .. } | Node::Timer { .. } => {
                if let Node::Entity { next, .. }
                | Node::Button { next, .. }
                | Node::Timer { next, .. } = &mut self.nodes[id]
                {
                    *next = value;
                }
            }
            Node::ButtonSet { .. } => {
                if let Node::ButtonSet { next, .. } = &mut self.nodes[id] {
                    *next = value;
                }
            }
            Node::Do { commands, .. } => match commands.last() {
                None => {
                    if let Node::Do { next, .. } = &mut self.nodes[id] {
                        *next = value;
                    }
                }
                Some(&last) => {
                    if !matches!(self.nodes[last], Node::ButtonSet { .. }) {
                        self.set_next(last, value);
                    }
                }
            },
            Node::DoRandom { commands, .. } => {
                if commands.is_empty() {
                    if let Node::DoRandom { next, .. } = &mut self.nodes[id] {
                        *next = value;
                    }
                    return;
                }
                for c in commands {
                    if !matches!(self.nodes[c], Node::ButtonSet { .. }) {
                        self.set_next(c, value);
                    }
                }
            }
            Node::If {
                then,
                otherwise,
                fail,
                ..
            } => {
                self.set_next(then, value);
                self.set_next(otherwise, value);
                if let Some(f) = fail {
                    self.set_next(f, value);
                }
            }
            Node::Switch { branches } => {
                for b in branches.into_iter().flatten() {
                    self.set_next(b, value);
                }
            }
            Node::CheckTime { then, .. } => {
                if let Node::CheckTime { next, .. } = &mut self.nodes[id] {
                    *next = value;
                }
                self.set_next(then, value);
            }
        }
    }
    /// INpcCommand.SetTalkingNpc: returns the NPC talking after the command.
    pub fn set_talking_npc(&mut self, id: NodeId, npc: &str) -> String {
        match self.nodes[id].clone() {
            Node::Entity { command, arg1, .. } => {
                let talking = if command == "change_npc" {
                    arg1
                } else {
                    Some(npc.to_owned())
                };
                if let Node::Entity { talking_npc, .. } = &mut self.nodes[id] {
                    *talking_npc = talking.clone();
                }
                talking.unwrap_or_default()
            }
            Node::Do { commands, .. } => {
                let mut current = npc.to_owned();
                for c in commands {
                    current = self.set_talking_npc(c, &current);
                }
                current
            }
            Node::DoRandom { commands, .. } => {
                let mut result = npc.to_owned();
                for c in commands {
                    result = self.set_talking_npc(c, npc);
                }
                result
            }
            Node::If {
                then,
                otherwise,
                fail,
                ..
            } => {
                let result = self.set_talking_npc(then, npc);
                self.set_talking_npc(otherwise, npc);
                if let Some(f) = fail {
                    self.set_talking_npc(f, npc);
                }
                result
            }
            Node::Switch { branches } => {
                for b in branches.into_iter().flatten() {
                    self.set_talking_npc(b, npc);
                }
                npc.to_owned()
            }
            Node::CheckTime { then, .. } => self.set_talking_npc(then, npc),
            Node::ButtonSet { buttons, .. } => {
                let mut result = npc.to_owned();
                for b in buttons {
                    if let Node::Button {
                        action: Some(a), ..
                    } = self.nodes[b]
                    {
                        result = self.set_talking_npc(a, npc);
                    }
                }
                result
            }
            Node::Button { action, .. } => match action {
                Some(a) => self.set_talking_npc(a, npc),
                None => npc.to_owned(),
            },
            Node::Timer { action, .. } => self.set_talking_npc(action, npc),
        }
    }
    /// AllCommandEntities in source enumeration order.
    pub fn entities(&self, id: NodeId, out: &mut Vec<NodeId>) {
        match &self.nodes[id] {
            Node::Entity { .. } => out.push(id),
            Node::Do { commands, .. } | Node::DoRandom { commands, .. } => {
                for &c in commands {
                    self.entities(c, out);
                }
            }
            Node::If {
                then,
                otherwise,
                fail,
                ..
            } => {
                self.entities(*then, out);
                self.entities(*otherwise, out);
                if let Some(f) = fail {
                    self.entities(*f, out);
                }
            }
            Node::Switch { branches } => {
                for b in branches.iter().flatten() {
                    self.entities(*b, out);
                }
            }
            Node::CheckTime { then, .. } => self.entities(*then, out),
            Node::ButtonSet { buttons, .. } => {
                for &b in buttons {
                    self.entities(b, out);
                }
            }
            Node::Button { action, .. } => {
                if let Some(a) = action {
                    self.entities(*a, out);
                }
            }
            Node::Timer { action, .. } => self.entities(*action, out),
        }
    }
    fn command_id(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id] {
            Node::Entity { id, .. } => id.as_deref(),
            _ => None,
        }
    }
}

/// (BuildingID, NpcID).
pub type NpcKey = (String, String);

#[derive(Clone, Debug)]
pub struct Talk {
    pub key: NpcKey,
    pub command: Option<String>,
    pub is_revealing: bool,
    pub content: NodeId,
}
impl Talk {
    /// Talk(parentPhase, xml); `default_building` is GetDefaultBuilding, or "t01_inn"
    /// while NpcBaseInfo itself is being built.
    fn load(
        arena: &mut Arena,
        xml: &Element,
        default_building: &dyn Fn(&str) -> String,
    ) -> Result<Self, ScriptError> {
        let command = xml.attribute("command").map(str::to_owned);
        let npc = xml
            .attribute("id")
            .ok_or(ScriptError("Talk without id"))?
            .to_owned();
        let building = xml
            .attribute("location")
            .map(str::to_owned)
            .unwrap_or_else(|| default_building(&npc));
        let first = xml
            .children
            .first()
            .ok_or(ScriptError("Talk without content"))?;
        let content = arena.load_command(first)?;
        let mut entities = Vec::new();
        arena.entities(content, &mut entities);
        let is_revealing = entities.iter().any(|&e| {
            matches!(&arena.nodes[e], Node::Entity { command, .. } if command == "reveal_storyline")
        });
        arena.set_talking_npc(content, &npc);
        Ok(Self {
            key: (building, npc),
            command,
            is_revealing,
            content,
        })
    }
}

/// One Default script of NpcTalkInfo.xml.
pub struct DefaultScript<'a> {
    pub feature: &'a str,
    pub root: &'a Element,
}

/// A default NPC and its talks by NpcCommand.
pub type DefaultNpc = (String, BTreeMap<String, Talk>);
/// NpcBaseInfo; maps keep the source Dictionary insertion order.
#[derive(Debug, Default)]
pub struct NpcBaseInfo {
    pub default_building: BTreeMap<String, String>,
    /// Buildings in insertion order, each with NPCs in insertion order and talks by command.
    pub default_npcs: Vec<(String, Vec<DefaultNpc>)>,
    pub server_commands: BTreeMap<String, NodeId>,
    pub button_sets: BTreeMap<String, NodeId>,
    pub default_button_set: Option<NodeId>,
    pub npc_features: BTreeMap<String, String>,
}
impl NpcBaseInfo {
    pub fn load(arena: &mut Arena, defaults: &[DefaultScript<'_>]) -> Result<Self, ScriptError> {
        let mut info = Self::default();
        for d in defaults {
            for b in d.root.children.iter().filter(|c| c.name == "ButtonSet") {
                let set = arena.load_command(b)?;
                let Node::ButtonSet { id: Some(id), .. } = &arena.nodes[set] else {
                    return Err(ScriptError("ButtonSet without id"));
                };
                let id = id.clone();
                if info.button_sets.insert(id.clone(), set).is_some() {
                    return Err(ScriptError("duplicate ButtonSet id"));
                }
                if id == "base_buttonset" {
                    info.default_button_set = Some(set);
                }
                let mut entities = Vec::new();
                arena.entities(set, &mut entities);
                info.register(arena, &entities)?;
            }
            for t in d.root.children.iter().filter(|c| c.name == "Talk") {
                let talk = Talk::load(arena, t, &|_| "t01_inn".to_owned())?;
                let (building, npc) = talk.key.clone();
                info.default_building
                    .entry(npc.clone())
                    .or_insert_with(|| building.clone());
                let command = talk
                    .command
                    .clone()
                    .ok_or(ScriptError("Talk without command"))?;
                let position = match info.default_npcs.iter().position(|(b, _)| *b == building) {
                    Some(p) => p,
                    None => {
                        info.default_npcs.push((building.clone(), Vec::new()));
                        info.default_npcs.len() - 1
                    }
                };
                let npcs = &mut info.default_npcs[position].1;
                let index = match npcs.iter().position(|(n, _)| *n == npc) {
                    Some(i) => i,
                    None => {
                        npcs.push((npc.clone(), BTreeMap::new()));
                        npcs.len() - 1
                    }
                };
                let mut entities = Vec::new();
                arena.entities(talk.content, &mut entities);
                if npcs[index].1.insert(command, talk).is_some() {
                    return Err(ScriptError("duplicate NPC command"));
                }
                info.register(arena, &entities)?;
                info.npc_features
                    .entry(npc)
                    .or_insert_with(|| d.feature.to_owned());
            }
        }
        if info.default_button_set.is_none() {
            return Err(ScriptError("base_buttonset is missing"));
        }
        Ok(info)
    }
    fn register(&mut self, arena: &Arena, entities: &[NodeId]) -> Result<(), ScriptError> {
        for &e in entities {
            if let Some(id) = arena.command_id(e)
                && self.server_commands.insert(id.to_owned(), e).is_some()
            {
                return Err(ScriptError("duplicate server command id"));
            }
        }
        Ok(())
    }
    /// GetDefaultBuilding: unknown or null NPCs are in "t01_inn".
    pub fn default_building(&self, npc: &str) -> String {
        self.default_building
            .get(npc)
            .cloned()
            .unwrap_or_else(|| "t01_inn".to_owned())
    }
    /// GetFeature: "" for NPCs without a Default feature.
    pub fn feature(&self, npc: &str) -> &str {
        self.npc_features.get(npc).map_or("", String::as_str)
    }
    pub fn command_talk(&self, building: &str, npc: &str, command: &str) -> Option<&Talk> {
        self.default_npcs
            .iter()
            .find(|(b, _)| b == building)?
            .1
            .iter()
            .find(|(n, _)| n == npc)?
            .1
            .get(command)
    }
}

/// StoryLineInfo row (HeroesContentsLoader.GetTable).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StoryLineInfo {
    pub id: String,
    pub xml_path: Option<String>,
    pub npc: Option<String>,
    pub category: Option<String>,
    pub reward_gold: i32,
    pub reward_exp: i32,
    pub reward_ap: i32,
    pub reward_title: i32,
    pub reward_item: Option<String>,
    pub reward_item_count: i32,
    pub reward_skill: Option<String>,
    pub reward_skill_rank: i32,
    pub auto_give_level: i32,
    pub class_restriction: i32,
    pub auto_process: bool,
    pub auto_reveal: bool,
    pub episode: Option<String>,
    pub feature: Option<String>,
}
impl StoryLineInfo {
    /// StoryContents.IsAllowedTo: (1 << class) & ClassRestriction.
    pub fn is_allowed_to(&self, class: i32) -> bool {
        1_i32.wrapping_shl(class as u32 & 31) & self.class_restriction != 0
    }
}

#[derive(Debug)]
pub struct Phase {
    pub id: String,
    pub goal: String,
    pub on_enter: Option<NodeId>,
    pub talks: Vec<Talk>,
    pub story_sector_talks: Vec<(String, Talk)>,
    pub deleted_npcs: Vec<NpcKey>,
    /// OnEvent handlers kept as their elements until events are migrated.
    pub events: Vec<Element>,
}
#[derive(Debug)]
pub struct StoryLine {
    pub info: StoryLineInfo,
    pub phases: Vec<Phase>,
    pub server_commands: BTreeMap<String, NodeId>,
}
impl StoryLine {
    pub fn load(
        arena: &mut Arena,
        info: StoryLineInfo,
        root: &Element,
        base: &NpcBaseInfo,
    ) -> Result<Self, ScriptError> {
        let mut phases: Vec<Phase> = Vec::new();
        for p in root.children.iter().filter(|c| c.name == "Phase") {
            let id = p
                .attribute("id")
                .ok_or(ScriptError("Phase without id"))?
                .to_owned();
            let goal = p.first("Goal").map(Element::value).unwrap_or_default();
            let on_enter = p
                .first("OnEnterPhase")
                .map(|e| arena.load_do(e))
                .transpose()?;
            let building = |npc: &str| base.default_building(npc);
            let talks = p
                .children
                .iter()
                .filter(|c| c.name == "Talk")
                .map(|t| Talk::load(arena, t, &building))
                .collect::<Result<Vec<_>, _>>()?;
            let mut story_sector_talks: Vec<(String, Talk)> = Vec::new();
            for t in p.children.iter().filter(|c| c.name == "StorySectorTalk") {
                let talk = Talk::load(arena, t, &building)?;
                if story_sector_talks.iter().any(|(n, _)| *n == talk.key.1) {
                    return Err(ScriptError("duplicate StorySectorTalk"));
                }
                story_sector_talks.push((talk.key.1.clone(), talk));
            }
            let mut deleted_npcs = Vec::new();
            for d in p.children.iter().filter(|c| c.name == "DeleteNpc") {
                let npc = d
                    .attribute("id")
                    .ok_or(ScriptError("DeleteNpc without id"))?;
                let location = d
                    .attribute("location")
                    .map(str::to_owned)
                    .unwrap_or_else(|| base.default_building(npc));
                let key = (location, npc.to_owned());
                if !deleted_npcs.contains(&key) {
                    deleted_npcs.push(key);
                }
            }
            let events = p
                .children
                .iter()
                .filter(|c| c.name == "OnEvent")
                .cloned()
                .collect::<Vec<_>>();
            for e in &events {
                validate_event(e)?;
            }
            if phases.iter().any(|q| q.id == id) {
                return Err(ScriptError("duplicate Phase"));
            }
            phases.push(Phase {
                id,
                goal,
                on_enter,
                talks,
                story_sector_talks,
                deleted_npcs,
                events,
            });
        }
        let mut server_commands = BTreeMap::new();
        for phase in &phases {
            let mut entities = Vec::new();
            if let Some(o) = phase.on_enter {
                arena.entities(o, &mut entities);
            }
            for t in &phase.talks {
                arena.entities(t.content, &mut entities);
            }
            for e in entities {
                if let Some(id) = arena.command_id(e)
                    && server_commands.insert(id.to_owned(), e).is_some()
                {
                    return Err(ScriptError("duplicate storyline server command id"));
                }
            }
        }
        Ok(Self {
            info,
            phases,
            server_commands,
        })
    }
    pub fn initial_phase(&self) -> Option<&Phase> {
        self.phases.first()
    }
    pub fn phase(&self, id: &str) -> Option<&Phase> {
        self.phases.iter().find(|p| p.id == id)
    }
}

/// EventHandler(xml) element and count checks.
fn validate_event(xml: &Element) -> Result<(), ScriptError> {
    let mut conditions = 0;
    let mut commands = 0;
    for c in &xml.children {
        match c.name.as_str() {
            "QuestCleared_Host" | "QuestCleared" | "QuestFailed" | "PvpCleared" | "PvpFailed"
            | "MonsterKilled" | "Level" => conditions += 1,
            "goto_phase" | "story_sector" => commands += 1,
            _ => return Err(ScriptError("invalid event element")),
        }
    }
    if commands == 0 || conditions == 0 {
        return Err(ScriptError("event without command or condition"));
    }
    Ok(())
}

/// NPCTalkInfo: base info plus every StoryLineInfo with an XmlPath, in table order.
#[derive(Debug, Default)]
pub struct NpcScripts {
    pub arena: Arena,
    pub base: NpcBaseInfo,
    pub story_lines: Vec<StoryLine>,
}
impl NpcScripts {
    pub fn load(
        defaults: &[DefaultScript<'_>],
        story_lines: Vec<(StoryLineInfo, Option<&Element>)>,
    ) -> Result<Self, ScriptError> {
        let mut arena = Arena::default();
        let base = NpcBaseInfo::load(&mut arena, defaults)?;
        let mut lines: Vec<StoryLine> = Vec::new();
        for (info, root) in story_lines {
            let Some(root) = root else {
                if info.xml_path.is_some() {
                    return Err(ScriptError("storyline XML is missing"));
                }
                continue;
            };
            if lines.iter().any(|l| l.info.id == info.id) {
                return Err(ScriptError("duplicate storyline"));
            }
            lines.push(StoryLine::load(&mut arena, info, root, &base)?);
        }
        Ok(Self {
            arena,
            base,
            story_lines: lines,
        })
    }
    pub fn story_line(&self, id: &str) -> Option<&StoryLine> {
        self.story_lines.iter().find(|l| l.info.id == id)
    }
    /// StoryEntity.NotifyEvent(Event_QuestFinished): for each progress, the first OnEvent
    /// handler of its phase whose conditions all hold runs its commands: goto_phase moves
    /// the progress (OnEnterPhase queries are not migrated and only counted) and
    /// story_sector sets the returned sector (−1 when absent or unparsable).
    pub fn quest_finished(&self, progress: &mut [Progress], event: &QuestFinished<'_>) -> EventOutcome {
        let mut outcome = EventOutcome {
            changed: false,
            story_sector: -1,
            skipped_on_enter: 0,
        };
        for p in progress.iter_mut() {
            let Some(line) = self.story_line(&p.story_line) else {
                continue;
            };
            let Some(phase) = p.phase.as_deref().and_then(|id| line.phase(id)) else {
                continue;
            };
            let Some(handler) = phase.events.iter().find(|h| {
                h.children
                    .iter()
                    .filter(|c| !matches!(c.name.as_str(), "goto_phase" | "story_sector"))
                    .all(|c| event.satisfies(c))
            }) else {
                continue;
            };
            let mut target = None;
            for c in &handler.children {
                let arg1 = c
                    .attribute("arg1")
                    .map(|a| a.trim().replace("\\n", "\n"))
                    .unwrap_or_else(|| {
                        c.text
                            .split(',')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .replace("\\n", "\n")
                    });
                match c.name.as_str() {
                    "goto_phase" => {
                        if let Some(next) = line.phase(&arg1) {
                            target = Some(next);
                        }
                    }
                    "story_sector" => outcome.story_sector = arg1.parse().unwrap_or(-1),
                    _ => {}
                }
                if let Some(next) = target.take() {
                    p.phase = Some(next.id.clone());
                    outcome.changed = true;
                    if next.on_enter.is_some() {
                        outcome.skipped_on_enter += 1;
                    }
                }
            }
        }
        outcome
    }
    /// GetAutoGiveStoryLines_ForLogin: AutoGiveStoryLines keys (first-seen level order)
    /// with level <= the character level, each list in table order.
    pub fn auto_give_for_login(&self, level: i32) -> Vec<&StoryLine> {
        let mut levels: Vec<i32> = Vec::new();
        for l in &self.story_lines {
            let a = l.info.auto_give_level;
            if a > 0 && !levels.contains(&a) {
                levels.push(a);
            }
        }
        levels
            .into_iter()
            .filter(|&a| a <= level)
            .flat_map(|a| {
                self.story_lines
                    .iter()
                    .filter(move |l| l.info.auto_give_level == a)
            })
            .collect()
    }
}

/// StoryProgress row of a character, in Entry.StoryProgress order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    pub story_line: String,
    pub phase: Option<String>,
    pub status: u8,
}

/// Event_QuestFinished of RewardCharacter.
#[derive(Clone, Copy, Debug)]
pub struct QuestFinished<'a> {
    pub quest: &'a str,
    pub success: bool,
    /// MicroPlayer.BossKilled: monster type → kills.
    pub boss_killed: &'a [(String, i32)],
    pub is_host: bool,
}
impl QuestFinished<'_> {
    /// IEventCondition.IsSatisfiedBy for the conditions an OnEvent element can hold.
    fn satisfies(&self, condition: &Element) -> bool {
        let value = condition.text.trim();
        match condition.name.as_str() {
            "QuestCleared_Host" => self.is_host && self.quest == value && self.success,
            "QuestCleared" => self.quest == value && self.success,
            "QuestFailed" => self.quest == value && !self.success,
            "MonsterKilled" => {
                let mut parts = condition.text.split(',');
                let (Some(monster), Some(count)) = (parts.next(), parts.next()) else {
                    return false;
                };
                let Ok(count) = count.trim().parse::<i32>() else {
                    return false;
                };
                self.boss_killed
                    .iter()
                    .find(|(m, _)| m == monster)
                    .map_or(0, |(_, n)| *n)
                    >= count
            }
            // PvpCleared/PvpFailed and Level hold only for their own events.
            _ => false,
        }
    }
}

/// NotifyEvent result: a progress moved, the story sector command and the OnEnterPhase
/// queries left unprocessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EventOutcome {
    pub changed: bool,
    pub story_sector: i32,
    pub skipped_on_enter: usize,
}
/// NpcInfo/StoryInfo projection of NpcManager.GetBuildingList.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NpcEntry {
    pub npc: String,
    pub feature: String,
    pub story_lines: Vec<(String, bool)>,
}
/// StoryLinesMessage row (BriefStoryLineInfo with PhaseText).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchedLine {
    pub story_line: String,
    pub phase: String,
    pub status: u8,
    pub goal: Option<String>,
}

impl NpcScripts {
    /// NPCTalkInfo.IsStoryEnable(StoryLineInfo).
    pub fn is_story_enabled(
        info: &StoryLineInfo,
        policy: &crate::domain::feature_policy::FeaturePolicy,
    ) -> Result<bool, ScriptError> {
        match info.feature.as_deref() {
            Some(feature) if !feature.is_empty() => policy
                .is_enabled(Some(feature))
                .map_err(|_| ScriptError("feature outside the recorded comparisons")),
            _ => Ok(true),
        }
    }
    fn progress_phase(&self, p: &Progress) -> Option<(&StoryLine, &Phase)> {
        let line = self.story_line(&p.story_line)?;
        let phase = line.phase(p.phase.as_deref()?)?;
        Some((line, phase))
    }
    /// NpcManager.Load then GetBuildingList for the character's progress.
    pub fn building_list(
        &self,
        progress: &[Progress],
        policy: &crate::domain::feature_policy::FeaturePolicy,
    ) -> Result<Vec<(String, Vec<NpcEntry>)>, ScriptError> {
        // Building.StoryLineNpcs: default NPCs first, then NPCs added by phase talks.
        type StoryNpc<'a> = (String, Vec<(String, &'a Talk)>);
        let mut buildings: Vec<(String, Vec<StoryNpc<'_>>)> = self
            .base
            .default_npcs
            .iter()
            .map(|(b, npcs)| {
                (
                    b.clone(),
                    npcs.iter().map(|(n, _)| (n.clone(), Vec::new())).collect(),
                )
            })
            .collect();
        let mut deleted: Vec<&NpcKey> = Vec::new();
        for p in progress {
            let Some((line, phase)) = self.progress_phase(p) else {
                continue;
            };
            for talk in &phase.talks {
                let Some((_, npcs)) = buildings.iter_mut().find(|(b, _)| *b == talk.key.0) else {
                    continue;
                };
                let npc = match npcs.iter().position(|(n, _)| *n == talk.key.1) {
                    Some(i) => i,
                    None => {
                        npcs.push((talk.key.1.clone(), Vec::new()));
                        npcs.len() - 1
                    }
                };
                let talks = &mut npcs[npc].1;
                match talks.iter_mut().find(|(l, _)| *l == line.info.id) {
                    Some(entry) => entry.1 = talk,
                    None => talks.push((line.info.id.clone(), talk)),
                }
            }
            deleted.extend(phase.deleted_npcs.iter());
        }
        let mut out = Vec::with_capacity(buildings.len());
        for (building, npcs) in buildings {
            let mut list = Vec::new();
            for (npc, talks) in npcs {
                if deleted.iter().any(|(b, n)| *b == building && *n == npc) {
                    continue;
                }
                let mut story_lines = Vec::new();
                for (line, talk) in talks {
                    let info = &self.story_line(&line).ok_or(ScriptError("storyline"))?.info;
                    if Self::is_story_enabled(info, policy)? {
                        story_lines.push((line, talk.is_revealing));
                    }
                }
                list.push(NpcEntry {
                    feature: self.base.feature(&npc).to_owned(),
                    npc,
                    story_lines,
                });
            }
            out.push((building, list));
        }
        Ok(out)
    }
    /// StoryEntity login: storylines OpenStoryLine would add (allowed class, not yet in
    /// progress), with the initial phase and status. A storyline without phases makes
    /// the source throw on InitialPhase.PhaseID.
    pub fn login_openings(
        &self,
        progress: &[Progress],
        level: i32,
        class: i32,
    ) -> Result<Vec<Progress>, ScriptError> {
        let mut opened: Vec<Progress> = Vec::new();
        for line in self.auto_give_for_login(level) {
            if !line.info.is_allowed_to(class)
                || progress
                    .iter()
                    .chain(&opened)
                    .any(|p| p.story_line == line.info.id)
            {
                continue;
            }
            let phase = line
                .initial_phase()
                .ok_or(ScriptError("storyline without phases"))?;
            // SetStoryLineStatus for AutoReveal: Category containing "Main" is Watching.
            let status = if line.info.auto_reveal {
                u8::from(
                    line.info
                        .category
                        .as_deref()
                        .is_some_and(|c| c.contains("Main")),
                )
            } else {
                255
            };
            opened.push(Progress {
                story_line: line.info.id.clone(),
                phase: Some(phase.id.clone()),
                status,
            });
        }
        Ok(opened)
    }
    /// StoryImpl.NotifyLevelUp: GetAutoGiveStoryLines_ForLevelUp(i) for i in (last, cur],
    /// opened when allowed to the class and not owned (OpenStoryLine success).
    pub fn level_up_openings(
        &self,
        progress: &[Progress],
        last: i32,
        cur: i32,
        class: i32,
    ) -> Result<Vec<Progress>, ScriptError> {
        let mut opened: Vec<Progress> = Vec::new();
        for level in last.saturating_add(1)..=cur {
            for line in self
                .story_lines
                .iter()
                .filter(|l| l.info.auto_give_level == level)
            {
                if !line.info.is_allowed_to(class)
                    || progress
                        .iter()
                        .chain(&opened)
                        .any(|p| p.story_line == line.info.id)
                {
                    continue;
                }
                opened.push(self.open_story_line(line)?);
            }
        }
        Ok(opened)
    }
    /// OpenStoryLine's new StoryProgress: initial phase and SetStoryLineStatus for
    /// AutoReveal (Category containing "Main" is Watching, otherwise Normal).
    pub fn open_story_line(&self, line: &StoryLine) -> Result<Progress, ScriptError> {
        let phase = line
            .initial_phase()
            .ok_or(ScriptError("storyline without phases"))?;
        let status = if line.info.auto_reveal {
            u8::from(
                line.info
                    .category
                    .as_deref()
                    .is_some_and(|c| c.contains("Main")),
            )
        } else {
            255
        };
        Ok(Progress {
            story_line: line.info.id.clone(),
            phase: Some(phase.id.clone()),
            status,
        })
    }
    /// SyncWatchingStoryLines with NewStoryLineDialog disabled (PhaseText is the Goal).
    pub fn watched_lines(
        &self,
        progress: &[Progress],
        policy: &crate::domain::feature_policy::FeaturePolicy,
    ) -> Result<Vec<WatchedLine>, ScriptError> {
        let dialog = policy
            .is_enabled(Some("NewStoryLineDialog"))
            .map_err(|_| ScriptError("NewStoryLineDialog"))?;
        let mut out = Vec::new();
        for p in progress {
            let Some((line, phase)) = self.progress_phase(p) else {
                continue;
            };
            if Self::is_story_enabled(&line.info, policy)? {
                out.push(WatchedLine {
                    story_line: p.story_line.clone(),
                    phase: phase.id.clone(),
                    status: p.status,
                    goal: (!dialog).then(|| phase.goal.clone()),
                });
            }
        }
        Ok(out)
    }
}
