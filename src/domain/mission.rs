//! Mission entry content of the MicroPlay service: QuestInfo.MakeDigest, the sector bases
//! built by MicroPlayContents (SectorBase.InitializeEntityString), and
//! SectorSequenceBuilder.GetSectorSequence / SectorSequence.GetInfo for the host's
//! GameStartGranted. Drops (evil cores, weak points, treasure maps, gold) are not ported:
//! ItemDropEntity and WeakPointEntity stay empty and TreasureBoxMaxCount is 0.
use super::feature_policy::FeaturePolicy;
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MissionError {
    #[error("mission content is outside the admitted profile")]
    Content,
    #[error("mission feature expression is outside the admitted profile")]
    Feature,
    #[error("quest has no sector information")]
    NoSectorInfo,
    #[error("mission form is not migrated")]
    Unsupported,
}

/// QuestInfo: ID, Level, TimeLimit, DropItemCountMin, DropItemCountMax, HuntingQuest,
/// InitGameTime, SectorMoveGameTime, GatheringRockMax, RequiredStoryLineID,
/// RequiredPhase, MaxMemberCount, IsSeason2.
pub type QuestRow = (
    Option<String>,
    i32,
    i32,
    i32,
    i32,
    bool,
    Option<i32>,
    Option<i32>,
    i32,
    Option<String>,
    Option<String>,
    i32,
    bool,
);
/// QuestGoalInfo: QuestID, ID, Relation, Target, GameSetting, GameSettingArg.
pub type GoalRow = (
    Option<String>,
    i32,
    i32,
    Option<String>,
    Option<String>,
    Option<i32>,
);
/// SectorGroupInfo: QuestID, FromGroup, FromSector, FromTrigger, ToGroup, ToSpawnPoint, Feature.
pub type SectorGroupRow = (
    Option<String>,
    i32,
    Option<i32>,
    Option<String>,
    i32,
    Option<String>,
    Option<String>,
);
/// Sectors: QuestID, Sequence, SectorID, BSPName, Weight.
pub type SectorRow = (Option<String>, i32, i32, Option<String>, i32);
/// An Entity of a quest sector with its EntityAttribute pairs.
pub type EntityRow = (i32, Vec<(Option<String>, Option<String>)>);
/// QuestSettingsInfo: RowID, ConditionID, Key, Value.
pub type SettingRow = (i32, Option<i32>, Option<String>, Option<String>);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MissionTables {
    pub quests: Vec<QuestRow>,
    pub goals: Vec<GoalRow>,
    /// Sectors: QuestID, Sequence, SectorID, BSPName, Weight.
    pub sectors: Vec<SectorRow>,
    pub sector_groups: Vec<SectorGroupRow>,
    pub tower_quests: Vec<String>,
    /// Entity rows of the quest sectors with their EntityAttribute pairs, provider order.
    pub entities: Vec<EntityRow>,
    /// NamedPropItemInfo entity names (NamedPropDropTable keys).
    pub named_props: Vec<String>,
    /// QuestSettingsInfo: RowID, ConditionID, Key, Value.
    pub settings: Vec<SettingRow>,
    /// QuestSettingsConditionInfo: ConditionID, Condition, Value.
    pub setting_conditions: Vec<(i32, Option<String>, Option<String>)>,
}

/// The unseeded System.Random instances of the builder; any uniform source is faithful.
pub trait MissionRandom {
    /// Random.Next(): [0, int.MaxValue).
    fn next(&mut self) -> i32;
    /// Random.Next(min, max): [min, max), min when the range is empty.
    fn next_range(&mut self, min: i32, max: i32) -> i32;
    /// Random.NextDouble(): [0, 1).
    fn next_double(&mut self) -> f64;
}

/// QuestDigest fields consumed by mission entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuestDigest {
    pub quest_id: String,
    pub quest_level: i32,
    pub item_count_min: i32,
    pub item_count_max: i32,
    pub time_limit: i32,
    pub is_hunting_quest: bool,
    pub party_limit: i32,
    pub init_game_time: i32,
    pub sector_move_game_time: i32,
    pub gathering_rock_max: i32,
    pub solo_sector: i32,
    pub hard_mode: bool,
    pub item_limit: String,
    pub gear_limit: String,
    pub is_season2: bool,
    pub required_story_line: Option<String>,
    pub required_phase: Option<String>,
}
impl QuestDigest {
    pub fn solo_mode(&self) -> bool {
        self.solo_sector > 0
    }
}

/// MapEntity: attribute pairs in EntityAttribute order and its serial number.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapEntity {
    pub id: i32,
    pub attributes: Vec<(String, String)>,
}
/// Char.IsWhiteSpace for the trimming of int.TryParse/String.Trim on ASCII content.
fn trim(v: &str) -> &str {
    v.trim_matches(|c: char| c.is_whitespace())
}
impl MapEntity {
    /// GetStringValue: first attribute with a case-insensitive key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }
    /// GetIntValue: int.TryParse of the first match, the default when it fails.
    pub fn get_int(&self, key: &str, default: i32) -> i32 {
        self.get(key)
            .and_then(|v| trim(v).parse::<i32>().ok())
            .unwrap_or(default)
    }
    pub fn get_float(&self, key: &str, default: f32) -> f32 {
        self.get(key)
            .and_then(|v| {
                let v = trim(v);
                v.bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
                    .then(|| v.parse::<f32>().ok())
                    .flatten()
            })
            .unwrap_or(default)
    }
    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        match self.get(key).map(trim) {
            Some("1") => true,
            Some("0") => false,
            _ => default,
        }
    }
    /// GetAllValues_StartsWith (culture-insensitive for the ASCII keys of the content).
    pub fn values_starting_with<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = &'a str> {
        self.attributes
            .iter()
            .filter(move |(k, _)| {
                k.len() >= prefix.len() && k[..prefix.len()].eq_ignore_ascii_case(prefix)
            })
            .map(|(_, v)| v.as_str())
    }
    /// MapEntity.ToEntityString with Environment.NewLine.
    pub fn entity_string(&self) -> String {
        let mut s = String::from("{\r\n");
        for (k, v) in &self.attributes {
            s.push_str(&format!("\t\"{k}\" \"{v}\"\r\n"));
        }
        s.push_str("}\r\n");
        s
    }
}

#[derive(Clone, Debug)]
pub struct Spawner {
    pub entity: usize,
    pub entity_id: i32,
    pub monster_entity_name: String,
    pub spawner_name: Option<String>,
    pub monster_types: Vec<String>,
    pub selected: usize,
    pub spawn_num: i32,
    pub base_drop_item: i32,
    pub is_elite: bool,
    pub is_boss: bool,
    pub is_named: bool,
    pub group_id: i32,
    pub respawnable: bool,
    pub respawn_time: i32,
    pub probability: f32,
    pub has_uniq_behaviors: bool,
    pub named_boss_probability: f32,
    pub named_variation: Option<String>,
    pub add_named_evil_core: i32,
    pub is_today_quest_candidate: bool,
    /// The spawned Monster's dead count; absent before Spawn.
    pub spawned: Option<i32>,
}
impl Spawner {
    pub fn monster_type(&self) -> &str {
        self.monster_types
            .get(self.selected)
            .map(String::as_str)
            .unwrap_or("")
    }
    fn current_spawn_count(&self) -> i32 {
        self.spawned.map_or(0, |dead| self.spawn_num - dead)
    }
    /// MonsterSpawner.ChangeNamedMonster.
    fn change_named(&mut self) -> bool {
        self.selected = 0;
        let Some(text) = self.monster_types.first() else {
            return false;
        };
        let parts: Vec<&str> = text.split(' ').collect();
        if parts.len() != 2 {
            return false;
        }
        let named = format!(
            "{} {}",
            parts[0],
            self.named_variation.as_deref().unwrap_or("")
        );
        self.monster_types = vec![named];
        self.base_drop_item += self.add_named_evil_core;
        self.is_named = true;
        true
    }
}

#[derive(Clone, Debug)]
pub enum SectorObject {
    Spawner(Spawner),
    /// MonsterGroup: GroupID (its entity string is never sent).
    MonsterGroup(i32),
    Trial {
        name: Option<String>,
        probability: i32,
        mods: Vec<i32>,
        group_number: i32,
        selected_mod: i32,
        send_to_host: bool,
    },
    Revenger {
        entity: usize,
        probability: f32,
    },
    TreasureBox {
        entity: usize,
        name: Option<String>,
    },
    NamedProp {
        entity: usize,
        name: String,
    },
    Gathering {
        entity: usize,
        name: Option<String>,
    },
    RandomBox {
        entity: usize,
        name: Option<String>,
    },
}
impl SectorObject {
    /// typeof(T).Name of the ISectorObject.
    fn type_name(&self) -> &'static str {
        match self {
            Self::Spawner(_) => "MonsterSpawner",
            Self::MonsterGroup(_) => "MonsterGroup",
            Self::Trial { .. } => "TrialFactor",
            Self::Revenger { .. } => "RevengerEvent",
            Self::TreasureBox { .. } => "TreasureBox",
            Self::NamedProp { .. } => "NamedProp",
            Self::Gathering { .. } => "GatheringRock",
            Self::RandomBox { .. } => "RandomBox",
        }
    }
    /// ISectorObject.Key (a null key is kept as None and compared as such).
    fn key(&self, entities: &[MapEntity]) -> Option<String> {
        match self {
            Self::Spawner(s) => Some(s.entity_id.to_string()),
            Self::MonsterGroup(g) => Some(g.to_string()),
            Self::Trial { name, .. } => name.clone(),
            Self::Revenger { entity, .. } => entities.get(*entity).map(|e| e.id.to_string()),
            Self::TreasureBox { name, .. }
            | Self::Gathering { name, .. }
            | Self::RandomBox { name, .. } => name.clone(),
            Self::NamedProp { name, .. } => Some(name.clone()),
        }
    }
    /// SectorBase.MapEntityTypes membership (sent to the host as entity strings).
    fn is_map_entity_type(&self) -> bool {
        !matches!(self, Self::MonsterGroup(_))
    }
    fn entity_string(&self, entities: &[MapEntity], practice: bool) -> Option<String> {
        let named = |entity: usize, name: Option<&str>| {
            let e = entities.get(entity)?;
            let mut s = String::from("{\r\n");
            for (k, v) in &e.attributes {
                if k.to_lowercase() != "entityname" {
                    s.push_str(&format!("\t\"{k}\" \"{v}\"\r\n"));
                }
            }
            s.push_str(&format!(
                "\t\"entityname\" \"{}\"\r\n}}\r\n",
                name.unwrap_or("")
            ));
            Some(s)
        };
        match self {
            Self::Spawner(sp) => {
                let e = entities.get(sp.entity)?;
                let mut s = String::from("{\r\n");
                s.push_str(&format!("\t\"templateName\" \"{}\"\r\n", sp.monster_type()));
                s.push_str(&format!(
                    "\t\"NPCTargetname\" \"{}\"\r\n",
                    sp.monster_entity_name
                ));
                s.push_str(&format!(
                    "\t\"CurrentSpawnCount\" \"{}\"\r\n",
                    sp.current_spawn_count()
                ));
                s.push_str(&format!(
                    "\t\"RespawnSpawnCount\" \"{}\"\r\n",
                    if sp.respawnable { sp.spawn_num } else { 0 }
                ));
                s.push_str(&format!("\t\"RespawnTime\" \"{}\"\r\n", sp.respawn_time));
                for (k, v) in &e.attributes {
                    let template = k.len() >= 12 && k[..12].eq_ignore_ascii_case("templateName");
                    if !template
                        && !matches!(
                            k.as_str(),
                            "probability"
                                | "create_named_probability"
                                | "named_behavior_1"
                                | "named_behavior_2"
                                | "named_behavior_3"
                        )
                    {
                        s.push_str(&format!("\t\"{k}\" \"{v}\"\r\n"));
                    }
                }
                s.push_str("}\r\n");
                Some(s)
            }
            Self::Revenger { entity, .. } => {
                let e = entities.get(*entity)?;
                let mut s = String::from("{\r\n");
                for (k, v) in &e.attributes {
                    if k != "probability" {
                        s.push_str(&format!("\t\"{k}\" \"{v}\"\r\n"));
                    }
                }
                s.push_str("}\r\n");
                Some(s)
            }
            Self::TreasureBox { entity, name }
            | Self::Gathering { entity, name }
            | Self::RandomBox { entity, name } => {
                if practice {
                    None
                } else {
                    named(*entity, name.as_deref())
                }
            }
            Self::NamedProp { entity, name } => {
                if practice {
                    None
                } else {
                    named(*entity, Some(name))
                }
            }
            Self::MonsterGroup(_) | Self::Trial { .. } => None,
        }
    }
}

/// RandomBox template: its box number and weight.
#[derive(Clone, Debug)]
pub struct RandomBoxBase {
    pub entity: usize,
    pub name: Option<String>,
    pub number: i32,
    pub weight: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Connection {
    pub from: i32,
    pub from_trigger: String,
    pub to: i32,
    pub to_spawn: String,
}

#[derive(Clone, Debug)]
pub struct SectorBase {
    pub sector_id: i32,
    pub bsp: Option<String>,
    pub group_id: i32,
    pub weight: i32,
    pub entity_string: String,
    pub objects: Vec<SectorObject>,
    /// IRandomSpawnMapEntityObject (random gathering rocks).
    pub random_spawn: Vec<SectorObject>,
    /// RandomBoxes by box number in first-appearance order.
    pub random_boxes: Vec<(i32, Vec<RandomBoxBase>)>,
    /// SectorConnectionInfo by trigger, insertion order.
    pub connections: Vec<(String, Connection)>,
    pub save_required: bool,
}

/// SectorSequenceBuilder of one quest: groups by Sequence in first-appearance order.
#[derive(Clone, Debug, Default)]
pub struct SectorBuilder {
    pub groups: Vec<(i32, Vec<SectorBase>)>,
    pub has_connection_info: bool,
}
impl SectorBuilder {
    fn add_connection(
        &mut self,
        from: i32,
        from_sector: Option<i32>,
        trigger: &str,
        to: i32,
        spawn: &str,
    ) -> Result<(), MissionError> {
        self.has_connection_info = true;
        let Some((_, sectors)) = self.groups.iter_mut().find(|(g, _)| *g == from) else {
            // Logged as invalidConnectionInfo by the original.
            return Ok(());
        };
        for sector in sectors {
            if from_sector.is_none_or(|s| s == sector.sector_id) {
                // Dictionary.Add: a duplicate trigger throws while loading the contents.
                if sector.connections.iter().any(|(t, _)| t == trigger) {
                    return Err(MissionError::Content);
                }
                sector.connections.push((
                    trigger.to_owned(),
                    Connection {
                        from,
                        from_trigger: trigger.to_owned(),
                        to,
                        to_spawn: spawn.to_owned(),
                    },
                ));
            }
        }
        Ok(())
    }
    fn add_base_connection(&mut self) -> Result<(), MissionError> {
        if !self.has_connection_info {
            let keys: Vec<i32> = self.groups.iter().map(|(g, _)| *g).collect();
            for key in keys {
                if keys_contains(&self.groups, key + 1) {
                    self.add_connection(key, None, "", key + 1, "")?;
                }
            }
            return Ok(());
        }
        for (_, sectors) in &mut self.groups {
            for sector in sectors {
                sector.save_required = true;
            }
        }
        Ok(())
    }
}
fn keys_contains(groups: &[(i32, Vec<SectorBase>)], key: i32) -> bool {
    groups.iter().any(|(g, _)| *g == key)
}

/// A sector object under its ISectorObject.Key.
pub type KeyedObject = (Option<String>, SectorObject);
/// A sector instance of a sequence: its group and the instanced sector objects.
#[derive(Clone, Debug)]
pub struct SectorInstance {
    pub group_id: i32,
    pub base: usize,
    /// SectorObjectDict: type name → (key, object), both in insertion order.
    pub objects: Vec<(&'static str, Vec<KeyedObject>)>,
}
impl SectorInstance {
    /// AddSectorObject: a duplicated key replaces the entry in place.
    fn add(&mut self, object: SectorObject, entities: &[MapEntity]) {
        let name = object.type_name();
        let key = object.key(entities);
        let index = match self.objects.iter().position(|(n, _)| *n == name) {
            Some(i) => i,
            None => {
                self.objects.push((name, Vec::new()));
                self.objects.len() - 1
            }
        };
        let rows = &mut self.objects[index].1;
        match rows.iter_mut().find(|(k, _)| *k == key) {
            Some(row) => row.1 = object,
            None => rows.push((key, object)),
        }
    }
    fn spawners_mut(&mut self) -> impl Iterator<Item = &mut Spawner> {
        self.objects
            .iter_mut()
            .flat_map(|(_, rows)| rows.iter_mut())
            .filter_map(|(_, o)| match o {
                SectorObject::Spawner(s) => Some(s),
                _ => None,
            })
    }
}

/// QuestSectorInfo of one sector instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorInfo {
    pub group_id: i32,
    pub bsp: Option<String>,
    pub entities: Vec<String>,
    pub connections: Vec<Connection>,
    /// TrialFactorInfo: TrialMod, GroupNumber, TrialName, SectorGroupID.
    pub trial_factors: Vec<(i32, i32, Option<String>, i32)>,
    /// TreasureBoxInfo: GroupID, EntityName (not visible, not opened).
    pub treasure_boxes: Vec<(i32, Option<String>)>,
    pub save_required: bool,
}
/// QuestSectorInfos of SectorSequence.GetInfo(isElection: false) before the first sector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectorInfos {
    pub current_group: i32,
    pub starting_group: i32,
    pub sectors: Vec<SectorInfo>,
}

/// SectorSequence of one MicroPlay.
#[derive(Clone, Debug)]
pub struct SectorSequence {
    pub starting_group: i32,
    pub sectors: Vec<SectorInstance>,
}

#[derive(Clone)]
struct QuestEntry {
    row: QuestRow,
    goals: Vec<GoalRow>,
    targets: Vec<Regex>,
}

/// MicroPlayContents/QuestContents slice used by mission entry.
pub struct MissionContents {
    quests: BTreeMap<String, QuestEntry>,
    builders: BTreeMap<String, SectorBuilder>,
    towers: HashSet<String>,
    entities: Vec<MapEntity>,
    settings: Vec<SettingRow>,
    setting_conditions: Vec<(i32, Option<String>, Option<String>)>,
}

const MAX_TARGET_PATTERN: usize = 512;

impl MissionContents {
    pub fn load(t: MissionTables, policy: &FeaturePolicy) -> Result<Self, MissionError> {
        let enabled = |f: Option<&str>| policy.is_enabled(f).map_err(|_| MissionError::Feature);
        let mut quests = BTreeMap::new();
        for row in t.quests {
            let id = row.0.clone().ok_or(MissionError::Content)?;
            if quests
                .insert(
                    id,
                    QuestEntry {
                        row,
                        goals: Vec::new(),
                        targets: Vec::new(),
                    },
                )
                .is_some()
            {
                return Err(MissionError::Content);
            }
        }
        for goal in t.goals {
            let Some(entry) = goal.0.as_deref().and_then(|q| quests.get_mut(q)) else {
                continue;
            };
            // QuestTargetDict: Relation 0 or 3 with a target, Regex(IgnoreCase).
            if matches!(goal.2, 0 | 3)
                && let Some(target) = &goal.3
            {
                if target.len() > MAX_TARGET_PATTERN {
                    return Err(MissionError::Content);
                }
                let regex = RegexBuilder::new(target)
                    .case_insensitive(true)
                    .size_limit(1 << 20)
                    .build()
                    .map_err(|_| MissionError::Content)?;
                entry.targets.push(regex);
            }
            entry.goals.push(goal);
        }
        let towers: HashSet<String> = t.tower_quests.into_iter().collect();
        // SectorBase per Sectors row (Sequence ≤ 0 are story sectors), grouped per quest.
        let mut builders: BTreeMap<String, SectorBuilder> = BTreeMap::new();
        let mut order: Vec<String> = Vec::new();
        for (quest, sequence, sector_id, bsp, weight) in t.sectors {
            let quest = quest.ok_or(MissionError::Content)?;
            if sequence <= 0 || towers.contains(&quest) {
                continue;
            }
            let builder = builders.entry(quest.clone()).or_insert_with(|| {
                order.push(quest.clone());
                SectorBuilder::default()
            });
            let base = SectorBase {
                sector_id,
                bsp,
                group_id: sequence,
                weight,
                entity_string: String::new(),
                objects: Vec::new(),
                random_spawn: Vec::new(),
                random_boxes: Vec::new(),
                connections: Vec::new(),
                save_required: false,
            };
            match builder.groups.iter_mut().find(|(g, _)| *g == sequence) {
                Some((_, sectors)) => sectors.push(base),
                None => builder.groups.push((sequence, vec![base])),
            }
        }
        for (quest, from, from_sector, trigger, to, spawn, feature) in t.sector_groups {
            if !enabled(feature.as_deref())? {
                continue;
            }
            if let Some(builder) = quest.as_deref().and_then(|q| builders.get_mut(q)) {
                builder.add_connection(
                    from,
                    from_sector,
                    trigger.as_deref().unwrap_or(""),
                    to,
                    spawn.as_deref().unwrap_or(""),
                )?;
            }
        }
        for builder in builders.values_mut() {
            builder.add_base_connection()?;
        }
        // Map entities per sector ID, in Entity order with their attribute order.
        let mut by_sector: BTreeMap<i32, Vec<Vec<(String, String)>>> = BTreeMap::new();
        for (sector, attributes) in t.entities {
            let attributes = attributes
                .into_iter()
                .map(|(k, v)| Ok((k.ok_or(MissionError::Content)?, v.unwrap_or_default())))
                .collect::<Result<Vec<_>, MissionError>>()?;
            by_sector.entry(sector).or_default().push(attributes);
        }
        let named_props: HashSet<String> = t.named_props.into_iter().collect();
        let gathering = enabled(Some("Gathering"))?;
        let gathering_rare = enabled(Some("GatheringRare"))?;
        let mut entities = Vec::new();
        let mut serial = 255_i32;
        for quest in &order {
            let targets = quests
                .get(quest)
                .map(|q| q.targets.clone())
                .unwrap_or_default();
            let Some(builder) = builders.get_mut(quest) else {
                continue;
            };
            for (_, sectors) in &mut builder.groups {
                for sector in sectors {
                    for attributes in by_sector.get(&sector.sector_id).into_iter().flatten() {
                        serial = serial.checked_add(1).ok_or(MissionError::Content)?;
                        entities.push(MapEntity {
                            id: serial,
                            attributes: attributes.clone(),
                        });
                        initialize_entity(
                            sector,
                            entities.len() - 1,
                            &entities,
                            &targets,
                            &named_props,
                            (gathering, gathering_rare),
                            &enabled,
                        )?;
                    }
                }
            }
        }
        Ok(Self {
            quests,
            builders,
            towers,
            entities,
            settings: t.settings,
            setting_conditions: t.setting_conditions,
        })
    }

    /// Quests with a normal sector sequence builder.
    pub fn quest_ids(&self) -> impl Iterator<Item = &str> {
        self.builders.keys().map(String::as_str)
    }

    pub fn entities(&self) -> &[MapEntity] {
        &self.entities
    }

    pub fn builder(&self, quest: &str) -> Option<&SectorBuilder> {
        self.builders.get(quest)
    }

    /// QuestInfo.MakeDigest(swearID) with the ItemLimit/GearLimit defaults.
    pub fn digest(&self, quest: &str, swear_id: i32) -> Result<QuestDigest, MissionError> {
        let entry = self.quests.get(quest).ok_or(MissionError::Content)?;
        let r = &entry.row;
        let mut d = QuestDigest {
            quest_id: quest.to_owned(),
            quest_level: r.1,
            item_count_min: r.3,
            item_count_max: r.4,
            time_limit: r.2,
            is_hunting_quest: r.5,
            party_limit: r.11,
            init_game_time: r.6.unwrap_or(-1),
            sector_move_game_time: r.7.unwrap_or(0),
            gathering_rock_max: r.8,
            solo_sector: 0,
            hard_mode: false,
            item_limit: String::new(),
            gear_limit: String::new(),
            is_season2: r.12,
            required_story_line: r.9.clone(),
            required_phase: r.10.clone(),
        };
        for g in &entry.goals {
            if g.2 != 2 || g.1 != swear_id {
                continue;
            }
            let arg = || g.5.ok_or(MissionError::Content);
            match g.4.as_deref() {
                None => {}
                Some("SoloMode") => {
                    d.solo_sector = arg()?;
                    d.item_count_min /= 2;
                    d.item_count_max /= 2;
                }
                Some("PartyLimit") => d.party_limit = arg()?,
                Some("HardMode") => d.hard_mode = true,
                Some("TimeLimit") => d.time_limit = arg()?,
                Some("ItemLimit") => d.item_limit = g.3.clone().unwrap_or_default(),
                Some("GearLimit") => d.gear_limit = g.3.clone().unwrap_or_default(),
                // BossTimeLimit and unknown settings do not reach mission entry.
                Some(_) => {}
            }
        }
        Ok(d)
    }

    /// QueryQuestDigestProcessor's ItemLimit rewrite for the selected difficulty.
    pub fn item_limit(&self, digest: &mut QuestDigest, swear_id: i32, difficulty: i32) {
        let Some(entry) = self.quests.get(&digest.quest_id) else {
            return;
        };
        let limited = entry
            .goals
            .iter()
            .any(|g| g.4.as_deref() == Some("ItemLimit") && g.5.is_some());
        if !limited {
            return;
        }
        let mut limit = String::new();
        for g in &entry.goals {
            if g.4.as_deref() != Some("ItemLimit") || !(g.2 == 0 || (g.2 == 2 && g.1 == swear_id)) {
                continue;
            }
            let mut text = g.3.clone().unwrap_or_default();
            if g.5.is_some_and(|a| a != difficulty) {
                text.clear();
            }
            if !text.is_empty() {
                if !limit.is_empty() {
                    limit.push('|');
                }
                limit.push_str(&text);
            }
        }
        digest.item_limit = limit;
    }

    /// QuestSettings keys of a quest and difficulty (QuestContents.GetQuestSettings).
    pub fn setting_keys(&self, quest: &str, difficulty: i32) -> Result<Vec<String>, MissionError> {
        let entry = self.quests.get(quest).ok_or(MissionError::Content)?;
        let (is_season2, max_member) = (entry.row.12, entry.row.11);
        let mut decided: BTreeMap<i32, bool> = BTreeMap::new();
        let mut keys = Vec::new();
        for (_, condition, key, _) in &self.settings {
            let pass = match condition {
                None => true,
                Some(c) => match decided.get(c) {
                    Some(v) => *v,
                    None => {
                        let mut ok = true;
                        for (_, kind, value) in self.setting_conditions.iter().filter(|r| r.0 == *c)
                        {
                            let value = value.as_deref().unwrap_or("");
                            let int = || {
                                value
                                    .trim()
                                    .parse::<i32>()
                                    .map_err(|_| MissionError::Content)
                            };
                            ok = match kind.as_deref() {
                                Some("QuestPrefix") => quest.starts_with(value),
                                Some("Difficulty") => difficulty == int()?,
                                Some("IsSeason2") => (int()? == 1) == is_season2,
                                Some("IsRaid") => (int()? == 1) == (max_member > 4),
                                _ => false,
                            };
                            if !ok {
                                break;
                            }
                        }
                        decided.insert(*c, ok);
                        ok
                    }
                },
            };
            if pass {
                keys.push(key.clone().unwrap_or_default());
            }
        }
        Ok(keys)
    }

    /// Fishing, hunting and tower quests use modules that are not migrated.
    pub fn check_supported(&self, quest: &str, difficulty: i32) -> Result<(), MissionError> {
        if self.towers.contains(quest) {
            return Err(MissionError::Unsupported);
        }
        let keys = self.setting_keys(quest, difficulty)?;
        if keys
            .iter()
            .any(|k| matches!(k.as_str(), "Fishing" | "Hunting" | "Tower"))
        {
            return Err(MissionError::Unsupported);
        }
        Ok(())
    }

    /// SectorSequenceBuilder.GetSectorSequence without the drop assignment.
    pub fn sector_sequence(
        &self,
        digest: &QuestDigest,
        practice: bool,
        today_quest_open: bool,
        random: &mut dyn MissionRandom,
    ) -> Result<SectorSequence, MissionError> {
        let builder = self
            .builders
            .get(&digest.quest_id)
            .ok_or(MissionError::NoSectorInfo)?;
        let starting_group = if digest.solo_mode() {
            digest.solo_sector
        } else {
            1
        };
        let mut sequence = SectorSequence {
            starting_group,
            sectors: Vec::new(),
        };
        for (group_id, sectors) in &builder.groups {
            let seed = random.next();
            if *group_id < starting_group {
                continue;
            }
            let base = pick_sector(sectors, seed).ok_or(MissionError::NoSectorInfo)?;
            let index = sectors
                .iter()
                .position(|s| std::ptr::eq(s, base))
                .ok_or(MissionError::NoSectorInfo)?;
            let instance = self.instance(*group_id, base, index, random)?;
            // SectorDict[GroupID] = sector.
            match sequence
                .sectors
                .iter_mut()
                .find(|s| s.group_id == *group_id)
            {
                Some(s) => *s = instance,
                None => sequence.sectors.push(instance),
            }
        }
        pick_named_monster(&mut sequence, today_quest_open, random);
        // MonsterSpawner.Spawn: the Monster starts with no dead count.
        for sector in &mut sequence.sectors {
            for spawner in sector.spawners_mut() {
                spawner.spawned = Some(0);
            }
        }
        if !practice {
            self.random_spawns(builder, &mut sequence, digest.gathering_rock_max, random);
            for sector in &mut sequence.sectors {
                let base = self.base(builder, sector)?;
                for (number, boxes) in &base.random_boxes {
                    if *number == -1 {
                        continue;
                    }
                    let total: i32 = boxes.iter().map(|b| b.weight).sum();
                    let mut roll = random.next_range(0, total);
                    for b in boxes {
                        if roll < b.weight {
                            sector.add(
                                SectorObject::RandomBox {
                                    entity: b.entity,
                                    name: b.name.clone(),
                                },
                                &self.entities,
                            );
                            break;
                        }
                        roll -= b.weight;
                    }
                }
            }
        }
        Ok(sequence)
    }

    fn base<'a>(
        &self,
        builder: &'a SectorBuilder,
        sector: &SectorInstance,
    ) -> Result<&'a SectorBase, MissionError> {
        builder
            .groups
            .iter()
            .find(|(g, _)| *g == sector.group_id)
            .and_then(|(_, s)| s.get(sector.base))
            .ok_or(MissionError::NoSectorInfo)
    }

    /// The SectorInstance constructor: ToInstance and the probability filter.
    fn instance(
        &self,
        group_id: i32,
        base: &SectorBase,
        index: usize,
        random: &mut dyn MissionRandom,
    ) -> Result<SectorInstance, MissionError> {
        let mut instance = SectorInstance {
            group_id,
            base: index,
            objects: Vec::new(),
        };
        for object in &base.objects {
            let object = match object {
                SectorObject::Spawner(s) => {
                    let mut s = s.clone();
                    s.selected = decide_spawn_monster(&s.monster_types, random);
                    s.spawned = None;
                    SectorObject::Spawner(s)
                }
                SectorObject::Trial {
                    name,
                    probability,
                    mods,
                    group_number,
                    ..
                } => {
                    let send_to_host = random.next_range(0, 100) - probability <= 0;
                    let pick = random.next_range(0, mods.len() as i32);
                    let selected_mod = *mods
                        .get(usize::try_from(pick).map_err(|_| MissionError::NoSectorInfo)?)
                        .ok_or(MissionError::NoSectorInfo)?;
                    SectorObject::Trial {
                        name: name.clone(),
                        probability: *probability,
                        mods: mods.clone(),
                        group_number: *group_number,
                        selected_mod,
                        send_to_host,
                    }
                }
                other => other.clone(),
            };
            let probability = match &object {
                SectorObject::Spawner(s) => Some(s.probability),
                SectorObject::Revenger { probability, .. } => Some(*probability),
                _ => None,
            };
            if let Some(p) = probability
                && p != -1.0
                && f64::from(p) < random.next_double()
            {
                continue;
            }
            instance.add(object, &self.entities);
        }
        Ok(instance)
    }

    /// Random gathering rocks: GatheringRockMax picks over every sector of the quest.
    fn random_spawns(
        &self,
        builder: &SectorBuilder,
        sequence: &mut SectorSequence,
        max: i32,
        random: &mut dyn MissionRandom,
    ) {
        let mut pool: Vec<(i32, usize, usize)> = Vec::new();
        for (group, sectors) in &builder.groups {
            for (si, s) in sectors.iter().enumerate() {
                for oi in 0..s.random_spawn.len() {
                    pool.push((*group, si, oi));
                }
            }
        }
        let mut chosen = Vec::new();
        let mut left = max - 1;
        while left >= 0 && !pool.is_empty() {
            let i = random.next_range(0, pool.len() as i32);
            let Ok(i) = usize::try_from(i) else { break };
            if i >= pool.len() {
                break;
            }
            chosen.push(pool.remove(i));
            left -= 1;
        }
        for sector in &mut sequence.sectors {
            let Some((_, sectors)) = builder.groups.iter().find(|(g, _)| *g == sector.group_id)
            else {
                continue;
            };
            let Some(base) = sectors.get(sector.base) else {
                continue;
            };
            for (oi, object) in base.random_spawn.iter().enumerate() {
                if chosen
                    .iter()
                    .any(|c| c.0 == sector.group_id && c.1 == sector.base && c.2 == oi)
                {
                    sector.add(object.clone(), &self.entities);
                }
            }
        }
    }

    /// SectorSequence.GetInfo(isElection: false) before any sector starts.
    pub fn info(
        &self,
        quest: &str,
        sequence: &SectorSequence,
        practice: bool,
    ) -> Result<SectorInfos, MissionError> {
        let builder = self.builders.get(quest).ok_or(MissionError::NoSectorInfo)?;
        let mut sectors = Vec::new();
        for sector in &sequence.sectors {
            let base = self.base(builder, sector)?;
            let mut entities = vec![base.entity_string.clone()];
            let mut trial_factors = Vec::new();
            let mut treasure_boxes = Vec::new();
            for (_, rows) in &sector.objects {
                for (_, object) in rows {
                    if object.is_map_entity_type()
                        && let Some(s) = object.entity_string(&self.entities, practice)
                    {
                        entities.push(s);
                    }
                }
            }
            for (_, rows) in &sector.objects {
                for (key, object) in rows {
                    if let SectorObject::Trial {
                        selected_mod,
                        group_number,
                        send_to_host: true,
                        ..
                    } = object
                    {
                        trial_factors.push((
                            *selected_mod,
                            *group_number,
                            key.clone(),
                            sector.group_id,
                        ));
                    }
                }
            }
            for (_, rows) in &sector.objects {
                for (_, object) in rows {
                    if let SectorObject::TreasureBox { name, .. } = object {
                        treasure_boxes.push((sector.group_id, name.clone()));
                    }
                }
            }
            sectors.push(SectorInfo {
                group_id: sector.group_id,
                bsp: base.bsp.clone(),
                entities,
                connections: base.connections.iter().map(|(_, c)| c.clone()).collect(),
                trial_factors,
                treasure_boxes,
                save_required: base.save_required,
            });
        }
        Ok(SectorInfos {
            current_group: -1,
            starting_group: sequence.starting_group,
            sectors,
        })
    }
}

/// SectorBaseGroup.GetSector without story drops (no StoryDropData is migrated).
fn pick_sector(sectors: &[SectorBase], seed: i32) -> Option<&SectorBase> {
    let sum: i32 = sectors.iter().map(|s| s.weight).sum();
    if sum == 0 {
        return None;
    }
    let roll = seed % sum;
    let mut acc = 0;
    for s in sectors {
        acc += s.weight;
        if acc > roll {
            return Some(s);
        }
    }
    None
}

/// MonsterSpawner.DecideSpawnMonster(0, 100), including its cumulative bookkeeping.
fn decide_spawn_monster(types: &[String], random: &mut dyn MissionRandom) -> usize {
    let num = random.next_range(0, 100);
    let mut num2 = 0_i32;
    for (i, t) in types.iter().enumerate() {
        let parts: Vec<&str> = t.split(' ').collect();
        if parts.len() > 2 {
            let mut result = trim(parts[2]).parse::<i32>().unwrap_or(0);
            result = result.wrapping_add(num2);
            if num2 <= num && num < result {
                return i;
            }
            num2 = num2.wrapping_add(result);
        }
    }
    0
}

/// SectorSequenceBuilder.PickNamedMonster.
fn pick_named_monster(
    sequence: &mut SectorSequence,
    today_quest_open: bool,
    random: &mut dyn MissionRandom,
) {
    let mut all: Vec<(usize, i32)> = Vec::new();
    let mut today: Vec<(usize, i32)> = Vec::new();
    for (si, sector) in sequence.sectors.iter_mut().enumerate() {
        for s in sector.spawners_mut() {
            if s.has_uniq_behaviors && s.named_boss_probability > 0.0 {
                all.push((si, s.entity_id));
                if s.is_today_quest_candidate {
                    today.push((si, s.entity_id));
                }
            }
        }
    }
    let mut named = false;
    if !today.is_empty() && today_quest_open {
        let i = random.next_range(0, today.len() as i32);
        if let Some(&(si, id)) = usize::try_from(i).ok().and_then(|i| today.get(i))
            && let Some(s) = sequence
                .sectors
                .get_mut(si)
                .and_then(|s| s.spawners_mut().find(|s| s.entity_id == id))
        {
            named = s.change_named();
        }
    }
    for (si, id) in all {
        if named {
            break;
        }
        if let Some(s) = sequence
            .sectors
            .get_mut(si)
            .and_then(|s| s.spawners_mut().find(|s| s.entity_id == id))
        {
            // ProceedProbability.
            if !s.has_uniq_behaviors || s.named_variation.as_deref().is_none_or(str::is_empty) {
                continue;
            }
            let roll = random.next_double() as f32;
            if s.named_boss_probability < roll {
                continue;
            }
            named = s.change_named();
        }
    }
}

/// SectorBase.InitializeEntityString for one map entity.
fn initialize_entity(
    sector: &mut SectorBase,
    index: usize,
    entities: &[MapEntity],
    targets: &[Regex],
    named_props: &HashSet<String>,
    (gathering, gathering_rare): (bool, bool),
    enabled: &dyn Fn(Option<&str>) -> Result<bool, MissionError>,
) -> Result<(), MissionError> {
    let e = entities.get(index).ok_or(MissionError::Content)?;
    let class = e.get("className").unwrap_or("").to_ascii_lowercase();
    let name = e.get("entityname").map(str::to_owned);
    let unique = class == "unique_npc_template_maker";
    let named = class == "named_boss_npc_template_maker";
    if gathering && class == "gathering_rock" {
        let rare = match &name {
            Some(n) => n.contains("rare"),
            // String.Contains on a null entityname throws in the original.
            None if !gathering_rare => return Err(MissionError::Content),
            None => false,
        };
        if gathering_rare || !rare {
            let rock = SectorObject::Gathering {
                entity: index,
                name,
            };
            if e.get("RandomSpawn")
                .is_some_and(|v| v.eq_ignore_ascii_case("1"))
            {
                sector.random_spawn.push(rock);
            } else {
                sector.objects.push(rock);
            }
        }
    } else if class == "npc_template_maker" || unique || named {
        if let Some(feature) = e.get("Feature").filter(|f| !f.is_empty())
            && !enabled(Some(feature))?
        {
            return Ok(());
        }
        let spawner = spawner_from(e, index, targets)?;
        if spawner.group_id != 0
            && !sector
                .objects
                .iter()
                .any(|o| matches!(o, SectorObject::MonsterGroup(g) if *g == spawner.group_id))
        {
            sector
                .objects
                .push(SectorObject::MonsterGroup(spawner.group_id));
        }
        sector.objects.push(SectorObject::Spawner(spawner));
    } else if class == "event_of_trial" {
        sector.objects.push(SectorObject::Trial {
            name: e.get("targetname").map(str::to_owned),
            probability: e.get_int("EventProbability", 0),
            mods: e
                .values_starting_with("TrialMod_")
                .filter_map(|v| trim(v).parse::<i32>().ok())
                .collect(),
            group_number: e.get_int("EffectiveGroup", 0),
            selected_mod: 0,
            send_to_host: false,
        });
    } else if class == "revenger_event" {
        sector.objects.push(SectorObject::Revenger {
            entity: index,
            probability: e.get_float("probability", 0.0),
        });
    } else if class == "treasure_box" {
        sector.objects.push(SectorObject::TreasureBox {
            entity: index,
            name,
        });
    } else if class == "random_box" {
        let b = RandomBoxBase {
            entity: index,
            name,
            number: e.get_int("boxnumber", -1),
            weight: e.get_int("boxweight", 0).wrapping_abs(),
        };
        match sector.random_boxes.iter_mut().find(|(n, _)| *n == b.number) {
            Some((_, list)) => list.push(b),
            None => sector.random_boxes.push((b.number, vec![b])),
        }
    } else if class == "math_counter" {
        sector.entity_string.push_str(&e.entity_string());
        sector.entity_string.push_str("\r\n");
    } else if let Some(n) = name.as_ref().filter(|n| named_props.contains(*n)) {
        // A repeated named prop gets "_" appended until it is unique in the sector.
        let mut unique_name = n.clone();
        while sector
            .objects
            .iter()
            .any(|o| matches!(o, SectorObject::NamedProp { name, .. } if *name == unique_name))
        {
            unique_name.push('_');
        }
        sector.objects.push(SectorObject::NamedProp {
            entity: index,
            name: unique_name,
        });
    } else {
        sector.entity_string.push_str(&e.entity_string());
        sector.entity_string.push_str("\r\n");
    }
    Ok(())
}

/// MonsterSpawner.From.
fn spawner_from(e: &MapEntity, index: usize, targets: &[Regex]) -> Result<Spawner, MissionError> {
    let mut types = Vec::new();
    for t in e.values_starting_with("templateName") {
        if trim(t).is_empty() {
            continue;
        }
        if t.split(' ').count() < 2 {
            // "invalid npc spawn target" aborts the contents load.
            return Err(MissionError::Content);
        }
        types.push(t.to_owned());
    }
    let group_id = e.get_int("GroupNumber", 0);
    let mut s = Spawner {
        entity: index,
        entity_id: e.id,
        monster_entity_name: e.id.to_string(),
        spawner_name: e.get("targetname").map(str::to_owned),
        monster_types: types,
        selected: 0,
        spawn_num: e.get_int("MaxNPCCount", 0),
        base_drop_item: e.get_int("DropItem", 0),
        is_elite: e.get_bool("SpawnElite", false),
        is_boss: false,
        is_named: false,
        group_id,
        respawnable: group_id != 0,
        respawn_time: e.get_int("RespawnTime", 300),
        probability: e.get_float("probability", -1.0),
        has_uniq_behaviors: e.get_bool("HasUniqBehavior", false),
        named_boss_probability: e.get_float("CreateNamedProbability", 0.0),
        named_variation: e.get("NamedVariation").map(str::to_owned),
        add_named_evil_core: e.get_int("AddNamedEvilCore", 0),
        is_today_quest_candidate: e.get_bool("IsTodayQuestCandidate", false),
        spawned: None,
    };
    'types: for t in &s.monster_types {
        let input = format!("kill {t}");
        for target in targets {
            if target.is_match(&input) {
                s.monster_entity_name.push_str("/Target");
                s.is_boss = true;
                break 'types;
            }
        }
    }
    Ok(s)
}
