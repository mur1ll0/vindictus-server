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
/// RequiredPhase, MaxMemberCount, IsSeason2, DisableUserShip, RequiredItem.
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
    bool,
    Option<String>,
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
    /// MonsterInfo: Identifier, Class, Variation, Difficulty, Exp, HP1, HP_Normal_Percent.
    pub monsters: Vec<MonsterRow>,
    /// MonsterDropItemInfo: MonsterID, QuestID, Difficulty, ItemClassEX, Weight, IsAbsolute,
    /// IsRequiredClass, IsUniqueDrop, Feature.
    pub monster_drops: Vec<DropRow>,
    /// QuestDifficultyInfo: QuestID, Difficulty, GameMode, EvilCoreCount, Feature.
    pub quest_difficulties: Vec<DifficultyRow>,
    /// ItemClassInfo of dropped classes: ItemClass, Rarity, ClassRestriction, Feature.
    pub drop_classes: Vec<(Option<String>, i32, i32, Option<String>)>,
}
/// QuestDifficultyInfo: QuestID, Difficulty, GameMode, EvilCoreCount, Feature.
pub type DifficultyRow = (Option<String>, i32, i32, i32, Option<String>);
/// MonsterDropItemInfo row.
pub type DropRow = (
    i32,
    Option<String>,
    i32,
    Option<String>,
    i32,
    bool,
    bool,
    bool,
    Option<String>,
);
/// MonsterInfo: Identifier, Class, Variation, Difficulty, Exp, HP1, HP_Normal_Percent.
pub type MonsterRow = (i32, Option<String>, Option<String>, i32, i32, i32, i32);

/// MonsterInfo values used by mission play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonsterInfo {
    pub identifier: i32,
    pub exp: i32,
    pub hp1: i32,
    pub hp_normal_percent: i32,
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
    pub disable_user_ship: bool,
    pub required_item: Option<String>,
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
    /// Monster.DropItem (cores per kill of an item drop entity).
    pub drop_item: i32,
    /// Monster.MonsterInfo at the play's difficulty.
    pub info: Option<MonsterInfo>,
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
    /// ItemDropEntity: MonsterEntityName → DropItem of monsters that drop cores.
    pub item_drop_entity: Vec<(String, i32)>,
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
    /// CurrentSector (its group), absent before the first MoveToNextSector.
    pub current: Option<i32>,
    /// CurrentSector.LastUsedSpawnPoint of the current sector.
    pub last_used_spawn_point: Option<String>,
    /// LatestClearedSectorIndex.
    pub latest_cleared: i32,
    /// PassedSector: SectorIDs already entered (AddPassedSectorInfo).
    pub passed: Vec<i32>,
    /// EvilCore sector objects of the solo player: (sector group, key, cores).
    pub cores: Vec<EvilCore>,
}

/// EvilCore of one player: ItemClassEX lists per EvilCoreType, in insertion order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvilCore {
    pub group: i32,
    pub key: String,
    pub is_boss: bool,
    pub items: Vec<(i32, Vec<String>)>,
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
    /// Monsters: MonsterType → (Difficulty, MonsterInfo) in insertion order.
    monsters: BTreeMap<String, Vec<(i32, MonsterInfo)>>,
    /// MonsterDropTable.DropTables: (MonsterID, Difficulty) → rows in provider order.
    drops: BTreeMap<(i32, i32), Vec<DropRow>>,
    /// QuestDifficultyInfoDic (feature-filtered): (QuestID, Difficulty) → (GameMode, EvilCoreCount).
    difficulties: BTreeMap<(String, i32), (i32, i32)>,
    /// itemClassInfoDic of dropped classes: ItemClass → (Rarity, ClassRestriction).
    drop_classes: BTreeMap<String, (i32, i32)>,
    /// FeatureMatrix: LuckyBonus, RareCoreEffectRarity, bossevilcore_bonus.
    lucky_bonus: bool,
    rare_core_rarity: i32,
    boss_evil_core_bonus: i32,
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
        // ToDoubleDictionary(MonsterType, Difficulty): a duplicate throws while loading.
        let mut monsters: BTreeMap<String, Vec<(i32, MonsterInfo)>> = BTreeMap::new();
        for (identifier, class, variation, difficulty, exp, hp1, hp_normal_percent) in t.monsters {
            let key = format!(
                "{} {}",
                class.as_deref().unwrap_or(""),
                variation.as_deref().unwrap_or("")
            );
            let rows = monsters.entry(key).or_default();
            if rows.iter().any(|(d, _)| *d == difficulty) {
                return Err(MissionError::Content);
            }
            rows.push((
                difficulty,
                MonsterInfo {
                    identifier,
                    exp,
                    hp1,
                    hp_normal_percent,
                },
            ));
        }
        let mut drops: BTreeMap<(i32, i32), Vec<DropRow>> = BTreeMap::new();
        for row in t.monster_drops {
            drops.entry((row.0, row.2)).or_default().push(row);
        }
        let mut difficulties = BTreeMap::new();
        for (quest, difficulty, game_mode, evil_core_count, feature) in t.quest_difficulties {
            if !enabled(feature.as_deref())? {
                continue;
            }
            let key = (quest.ok_or(MissionError::Content)?, difficulty);
            if difficulties
                .insert(key, (game_mode, evil_core_count))
                .is_some()
            {
                return Err(MissionError::Content);
            }
        }
        // ToDictionary_OverwriteDuplicated over the feature-enabled rows.
        let mut drop_classes = BTreeMap::new();
        for (class, rarity, restriction, feature) in t.drop_classes {
            if enabled(feature.as_deref())? {
                drop_classes.insert(class.ok_or(MissionError::Content)?, (rarity, restriction));
            }
        }
        let lucky_bonus = enabled(Some("LuckyBonus"))?;
        let rare_core_rarity = policy
            .get_integer("RareCoreEffectRarity")
            .map_err(|_| MissionError::Feature)?;
        let boss_evil_core_bonus = policy
            .get_integer("bossevilcore_bonus")
            .map_err(|_| MissionError::Feature)?;
        Ok(Self {
            quests,
            builders,
            towers,
            entities,
            settings: t.settings,
            setting_conditions: t.setting_conditions,
            monsters,
            drops,
            difficulties,
            drop_classes,
            lucky_bonus,
            rare_core_rarity,
            boss_evil_core_bonus,
        })
    }

    /// MicroPlayContents.GetMonsterInfo: "class variation [weight]" without the weight,
    /// the only row or difficulty -1 first, else the difficulty row or its -1 row.
    pub fn monster_info(&self, monster_type: &str, difficulty: i32) -> Option<MonsterInfo> {
        let parts: Vec<&str> = monster_type.split(' ').collect();
        let key = match parts.len() {
            0..=2 => monster_type.to_owned(),
            // GetAvailableMonsterType throws (logged) for these and keeps the type.
            3 if trim(parts[2]).parse::<i32>().is_ok() => format!("{} {}", parts[0], parts[1]),
            _ => monster_type.to_owned(),
        };
        let rows = self.monsters.get(&key)?;
        if difficulty == -1 || rows.len() == 1 {
            return rows.first().map(|(_, i)| *i);
        }
        rows.iter()
            .find(|(d, _)| *d == difficulty)
            .or_else(|| rows.iter().find(|(d, _)| *d == -1))
            .map(|(_, i)| *i)
    }

    /// Whether QuestDifficultyInfoDic has the (enabled) difficulty of the quest.
    pub fn has_difficulty(&self, quest: &str, difficulty: i32) -> bool {
        self.difficulties
            .contains_key(&(quest.to_owned(), difficulty))
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
            disable_user_ship: r.13,
            required_item: r.14.clone(),
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

    /// Whether the quest has the swear goal (Relation 2) of this ID.
    pub fn has_swear(&self, quest: &str, swear_id: i32) -> bool {
        self.quests
            .get(quest)
            .is_some_and(|q| q.goals.iter().any(|g| g.2 == 2 && g.1 == swear_id))
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

    /// QuestSettings of a quest and difficulty (QuestContents.GetQuestSettings): the
    /// (Key, Value) rows whose condition holds, in RowID dictionary order.
    pub fn settings(
        &self,
        quest: &str,
        difficulty: i32,
    ) -> Result<Vec<(String, String)>, MissionError> {
        let entry = self.quests.get(quest).ok_or(MissionError::Content)?;
        let (is_season2, max_member) = (entry.row.12, entry.row.11);
        let mut decided: BTreeMap<i32, bool> = BTreeMap::new();
        let mut rows = Vec::new();
        for (_, condition, key, value) in &self.settings {
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
                rows.push((
                    key.clone().unwrap_or_default(),
                    value.clone().unwrap_or_default(),
                ));
            }
        }
        Ok(rows)
    }

    /// Fishing, hunting and tower quests (and the selected sub-weapon of fishing) use
    /// modules that are not migrated.
    pub fn check_supported(&self, quest: &str, difficulty: i32) -> Result<(), MissionError> {
        if self.towers.contains(quest) {
            return Err(MissionError::Unsupported);
        }
        let settings = self.settings(quest, difficulty)?;
        if settings.iter().any(|(k, _)| {
            matches!(
                k.as_str(),
                "Fishing" | "Hunting" | "Tower" | "SelectedSubWeapon"
            )
        }) {
            return Err(MissionError::Unsupported);
        }
        Ok(())
    }

    /// SectorSequenceBuilder.GetSectorSequence (treasure maps, HugeLucky, story and event
    /// drops are not migrated).
    pub fn sector_sequence(
        &self,
        digest: &QuestDigest,
        difficulty: i32,
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
            current: None,
            last_used_spawn_point: None,
            latest_cleared: -1,
            passed: Vec::new(),
            cores: Vec::new(),
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
        // MonsterSpawner.Spawn: the Monster starts with no dead count and the MonsterInfo of
        // the play's difficulty.
        for sector in &mut sequence.sectors {
            for spawner in sector.spawners_mut() {
                spawner.spawned = Some(0);
                spawner.info = self.monster_info(spawner.monster_type(), difficulty);
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
            self.assign_drops(digest, difficulty, &mut sequence, random)?;
        }
        Ok(sequence)
    }

    /// GetSectorSequence's drop assignment: TotalItemCountDice picks ItemCountMin..=Max
    /// drop slots among the non-boss monsters without a base drop whose MonsterDropTable
    /// exists (partial shuffle); bosses and unique-behavior monsters get their base drop
    /// plus the difficulty's EvilCoreCount and bossevilcore_bonus (1 + bonus in GameMode 4).
    fn assign_drops(
        &self,
        digest: &QuestDigest,
        difficulty: i32,
        sequence: &mut SectorSequence,
        random: &mut dyn MissionRandom,
    ) -> Result<(), MissionError> {
        let mut wanted = random.next_range(digest.item_count_min, digest.item_count_max + 1);
        let difficulty_info = self
            .difficulties
            .get(&(digest.quest_id.clone(), difficulty))
            .copied();
        let bonus = self.boss_evil_core_bonus;
        let mut candidates: Vec<(usize, i32)> = Vec::new();
        let mut has_table = false;
        for (si, sector) in sequence.sectors.iter_mut().enumerate() {
            for s in sector.spawners_mut() {
                if s.spawned.is_none() {
                    continue;
                }
                if let Some(info) = s.info {
                    has_table = self.drops.contains_key(&(info.identifier, difficulty));
                }
                let boosted = |base: i32| -> Result<i32, MissionError> {
                    let (mode, evil_cores) = difficulty_info.ok_or(MissionError::NoSectorInfo)?;
                    Ok(if mode == 4 {
                        1 + bonus
                    } else {
                        base + evil_cores + bonus
                    })
                };
                if s.base_drop_item == 0 && has_table {
                    candidates.push((si, s.entity_id));
                } else if s.is_boss {
                    s.drop_item = if s.base_drop_item == 0 {
                        0
                    } else {
                        boosted(s.base_drop_item)?
                    };
                } else if s.has_uniq_behaviors {
                    s.drop_item = boosted(s.base_drop_item)?;
                } else {
                    s.drop_item = s.base_drop_item;
                }
            }
        }
        if (candidates.len() as i32) < wanted {
            wanted = candidates.len() as i32;
        }
        // ShuffleFirstNthElement(SelectDropMonsterDice, n).
        for i in 0..usize::try_from(wanted).unwrap_or(0) {
            let j = random.next_range(i as i32, candidates.len() as i32);
            candidates.swap(i, usize::try_from(j).map_err(|_| MissionError::Content)?);
        }
        for &(si, id) in candidates.iter().take(usize::try_from(wanted).unwrap_or(0)) {
            if let Some(s) = sequence
                .sectors
                .get_mut(si)
                .and_then(|s| s.spawners_mut().find(|s| s.entity_id == id))
            {
                s.drop_item = 1;
            }
        }
        Ok(())
    }

    /// MonsterDropTableInstance.PickItem for the Fiona (BaseCharacter 1): absolute rows
    /// roll out of 1,000,000 first, then the relational weights; required-class rows need
    /// the class restriction bit; unique rows are picked once per table instance.
    fn pick_drop(
        &self,
        rows: &[&DropRow],
        unique_got: &mut Vec<String>,
        random: &mut dyn MissionRandom,
    ) -> Option<String> {
        let mut absolute: Vec<&DropRow> = Vec::new();
        let mut relational: Vec<&DropRow> = Vec::new();
        let mut total: i32 = 0;
        for row in rows {
            let class_ex = row.3.as_deref().unwrap_or("");
            if row.7 && unique_got.iter().any(|u| u == class_ex) {
                continue;
            }
            if row.6 {
                let Some((_, restriction)) = self.drop_classes.get(item_class(class_ex)) else {
                    continue;
                };
                if (1 << 1) & restriction == 0 {
                    continue;
                }
            }
            if row.5 {
                absolute.push(row);
                continue;
            }
            relational.push(row);
            total = total.wrapping_add(row.4);
        }
        let mut chosen = |row: &DropRow| -> Option<String> {
            let class_ex = row.3.clone()?;
            if row.7 && !unique_got.contains(&class_ex) {
                unique_got.push(class_ex.clone());
            }
            Some(class_ex)
        };
        if !absolute.is_empty() {
            let mut roll = random.next_range(0, 1_000_000);
            for row in &absolute {
                roll -= row.4;
                if roll < 0 {
                    return chosen(row);
                }
            }
        }
        let mut roll = random.next_range(0, total);
        for row in &relational {
            roll -= row.4;
            if roll < 0 {
                return chosen(row);
            }
        }
        None
    }

    /// EvilCore.IsRareCore: the ItemClassInfo rarity reaches RareCoreEffectRarity.
    pub fn is_rare_core(&self, class_ex: &str) -> bool {
        self.rare_core_rarity > 0
            && self
                .drop_classes
                .get(item_class(class_ex))
                .is_some_and(|(rarity, _)| *rarity >= self.rare_core_rarity)
    }

    /// Monster.MakeEvilCore for the solo player of a killed item-drop monster: DropItem
    /// picks from its MonsterDropTable (javelin_lvl0 without a table) and a LuckyCoreDice
    /// gold core per pick. Returns the additional cores to announce (DropGoldCore).
    #[allow(clippy::too_many_arguments)]
    pub fn make_evil_core(
        &self,
        sequence: &mut SectorSequence,
        key: &str,
        drop_item: i32,
        info: Option<MonsterInfo>,
        is_boss: bool,
        quest: &str,
        difficulty: i32,
        luck: i32,
        random: &mut dyn MissionRandom,
    ) -> Result<Vec<(i32, bool)>, MissionError> {
        let group = sequence.current.ok_or(MissionError::NoSectorInfo)?;
        let rows: Option<Vec<&DropRow>> = info
            .and_then(|i| self.drops.get(&(i.identifier, difficulty)))
            .map(|rows| {
                rows.iter()
                    .filter(|r| r.1.as_deref().is_none_or(|q| q == quest))
                    .collect()
            });
        // MonsterDropTableInstance.From returns null for an empty filtered list.
        let rows = rows.filter(|r| !r.is_empty());
        let mut unique_got = Vec::new();
        let mut items: Vec<(i32, Vec<String>)> = Vec::new();
        let add = |items: &mut Vec<(i32, Vec<String>)>, kind: i32, class: Option<String>| {
            let Some(class) = class else { return };
            match items.iter_mut().find(|(k, _)| *k == kind) {
                Some((_, list)) => list.push(class),
                None => items.push((kind, vec![class])),
            }
        };
        let mut additional = Vec::new();
        for order in 0..drop_item.max(0) {
            let text = match &rows {
                Some(rows) => self.pick_drop(rows, &mut unique_got, random),
                None => Some("javelin_lvl0".to_owned()),
            };
            add(&mut items, EVIL_CORE, text);
            if self.lucky_roll(order, is_boss, luck, random) {
                // The original dereferences the null table here.
                let rows = rows.as_ref().ok_or(MissionError::Content)?;
                let gold = self.pick_drop(rows, &mut unique_got, random);
                let rare = gold.as_deref().is_some_and(|g| self.is_rare_core(g));
                add(&mut items, BASIC_GOLD_CORE, gold);
                additional.push((BASIC_GOLD_CORE, rare));
            }
        }
        if !items.is_empty() {
            match sequence
                .cores
                .iter_mut()
                .find(|c| c.group == group && c.key == key)
            {
                Some(core) => {
                    for (kind, list) in items {
                        match core.items.iter_mut().find(|(k, _)| *k == kind) {
                            Some((_, l)) => l.extend(list),
                            None => core.items.push((kind, list)),
                        }
                    }
                }
                None => sequence.cores.push(EvilCore {
                    group,
                    key: key.to_owned(),
                    is_boss,
                    items,
                }),
            }
        }
        Ok(additional)
    }

    /// LuckyCoreDice.Roll.
    fn lucky_roll(
        &self,
        order: i32,
        is_boss: bool,
        luck: i32,
        random: &mut dyn MissionRandom,
    ) -> bool {
        if !self.lucky_bonus {
            return false;
        }
        if order == 0 && is_boss && luck > 100 {
            return true;
        }
        let luck = luck.clamp(100, 150);
        random.next_range(0, 100_000) < (luck - 100) * 1500
    }

    /// EvilCore.Loot for the solo player: the first core of the kicked type matching the
    /// rare/normal request (else the last one) is removed and returned. `Err` keeps the
    /// core (a class without ItemClassInfo).
    pub fn loot(
        &self,
        sequence: &mut SectorSequence,
        key: &str,
        kind: i32,
        rare: bool,
    ) -> Result<Option<String>, MissionError> {
        let Some(group) = sequence.current else {
            return Ok(None);
        };
        let Some(ci) = sequence
            .cores
            .iter()
            .position(|c| c.group == group && c.key == key)
        else {
            return Ok(None);
        };
        let core = &mut sequence.cores[ci];
        let Some(li) = core.items.iter().position(|(k, _)| *k == kind) else {
            return Ok(None);
        };
        let list = &core.items[li].1;
        let mut index = None;
        for (i, class_ex) in list.iter().enumerate() {
            if !self.drop_classes.contains_key(item_class(class_ex)) {
                return Err(MissionError::Content);
            }
            if self.is_rare_core(class_ex) == rare {
                index = Some(i);
                break;
            }
        }
        let index = index.unwrap_or(list.len().saturating_sub(1));
        let list = &mut core.items[li].1;
        if list.is_empty() {
            return Ok(None);
        }
        let item = list.remove(index);
        if list.is_empty() {
            core.items.remove(li);
        }
        Ok(Some(item))
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

    /// SectorSequence.MoveNext: the starting group first, then the connection of the
    /// trigger (or the "" connection) to the target group (-1 accepts any). Entering a
    /// sector records it as passed (MicroPlay.OnMoveSector → AddPassedSectorInfo).
    pub fn move_next(
        &self,
        quest: &str,
        sequence: &mut SectorSequence,
        trigger: &str,
        target_group: i32,
    ) -> Result<bool, MissionError> {
        let builder = self.builders.get(quest).ok_or(MissionError::NoSectorInfo)?;
        let (to, spawn) = match sequence.current {
            None => (sequence.starting_group, String::new()),
            Some(group) => {
                sequence.latest_cleared = group - 1;
                let current = sequence
                    .sectors
                    .iter()
                    .find(|s| s.group_id == group)
                    .ok_or(MissionError::NoSectorInfo)?;
                let base = self.base(builder, current)?;
                let connection = base
                    .connections
                    .iter()
                    .find(|(t, _)| t == trigger)
                    .or_else(|| base.connections.iter().find(|(t, _)| t.is_empty()));
                let Some((_, c)) = connection else {
                    return Ok(false);
                };
                (c.to, c.to_spawn.clone())
            }
        };
        let Some(next) = sequence.sectors.iter().find(|s| s.group_id == to) else {
            return Ok(false);
        };
        if next.group_id != target_group && target_group != -1 {
            return Ok(false);
        }
        let sector_id = self.base(builder, next)?.sector_id;
        sequence.current = Some(to);
        sequence.last_used_spawn_point = Some(spawn);
        if !sequence.passed.contains(&sector_id) {
            sequence.passed.push(sector_id);
        }
        Ok(true)
    }

    /// MonsterKilledProcessor's lookup and Monster.MonsterKilled bookkeeping: the live
    /// Monster of the current sector (spawned and not all dead) gets one more dead.
    pub fn kill(&self, sequence: &mut SectorSequence, target: i32) -> Kill {
        let Some(group) = sequence.current else {
            return Kill::NotInSector;
        };
        let Some(sector) = sequence.sectors.iter_mut().find(|s| s.group_id == group) else {
            return Kill::NotInSector;
        };
        let Some(spawner) = sector.spawners_mut().find(|s| s.entity_id == target) else {
            return Kill::Unknown;
        };
        let Some(dead) = spawner.spawned else {
            return Kill::Unknown;
        };
        if dead >= spawner.spawn_num {
            return Kill::AlreadyDead;
        }
        spawner.spawned = Some(dead + 1);
        Kill::Killed {
            monster_type: spawner.monster_type().to_owned(),
            is_boss: spawner.is_boss,
            is_named: spawner.is_named,
            spawn_count: 1,
            all_dead: dead + 1 >= spawner.spawn_num,
            key: spawner.monster_entity_name.clone(),
            drop_item: spawner.drop_item,
            info: spawner.info,
        }
    }

    /// Monster.MonsterKilled's DropItem reset of an item drop entity that is not all dead.
    pub fn clear_drop(&self, sequence: &mut SectorSequence, target: i32) {
        let Some(group) = sequence.current else {
            return;
        };
        if let Some(s) = sequence
            .sectors
            .iter_mut()
            .find(|s| s.group_id == group)
            .and_then(|s| s.spawners_mut().find(|s| s.entity_id == target))
        {
            s.drop_item = 0;
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
            // ItemDropEntity: the live Monsters (spawned, not all dead) with DropItem > 0.
            let item_drop_entity = sector
                .objects
                .iter()
                .flat_map(|(_, rows)| rows.iter())
                .filter_map(|(_, o)| match o {
                    SectorObject::Spawner(s)
                        if s.drop_item > 0 && s.spawned.is_some_and(|d| d < s.spawn_num) =>
                    {
                        Some((s.monster_entity_name.clone(), s.drop_item))
                    }
                    _ => None,
                })
                .collect();
            sectors.push(SectorInfo {
                group_id: sector.group_id,
                bsp: base.bsp.clone(),
                entities,
                connections: base.connections.iter().map(|(_, c)| c.clone()).collect(),
                trial_factors,
                treasure_boxes,
                save_required: base.save_required,
                item_drop_entity,
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
        drop_item: 0,
        info: None,
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

/// QuestSettings.QuickSlotMultiplier / SubWeaponMultiplier: 0 with the Set*Limit key,
/// otherwise the product of float.Parse of every matching value.
pub fn multiplier(
    settings: &[(String, String)],
    key: &str,
    limit: &str,
) -> Result<f32, MissionError> {
    if settings.iter().any(|(k, _)| k == limit) {
        return Ok(0.0);
    }
    let mut product = 1.0_f32;
    for (k, v) in settings {
        if k == key {
            let v = trim(v);
            if v.is_empty()
                || !v
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'+'))
            {
                return Err(MissionError::Content);
            }
            product *= v.parse::<f32>().map_err(|_| MissionError::Content)?;
        }
    }
    Ok(product)
}

/// What a quick slot resolves to for ItemOwner.OnMakeConsumablesInfo.
#[derive(Clone, Debug)]
pub enum SlotSource {
    /// A "skill_" slot: SkillInfo presence and IsActiveSkill, and the character's rank.
    Skill {
        info: Option<bool>,
        rank: Option<i32>,
    },
    /// An item slot: ItemClassInfo (absent → skipped) and the usable item count.
    Item {
        info: Option<ItemSlotInfo>,
        usable: i32,
    },
}
#[derive(Clone, Debug)]
pub struct ItemSlotInfo {
    pub class: String,
    pub sub_weapon: bool,
    pub quick_slot_max_stack: i32,
    pub draft: i32,
    pub alchemist_box: bool,
}
/// ConsumablesInfo of the battle inventory (UsedNum 0, no inner consumables).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consumable {
    pub class: String,
    pub bring: i32,
    pub draft: i32,
}

/// OnMakeConsumablesInfo for the equipped quick slots, then AddPlayer's BattleInventory
/// (ItemLimit or BringNum 0 → empty consumable) with SetConsumable on a .NET Dictionary.
pub fn battle_inventory(
    slots: Vec<(i32, String, SlotSource)>,
    quick_multiplier: f32,
    sub_weapon_multiplier: f32,
    item_limit: Option<&Regex>,
) -> Result<Vec<(i32, Consumable)>, MissionError> {
    let mut made: Vec<(i32, Consumable)> = Vec::new();
    let mut loaded: Vec<(String, i32)> = Vec::new();
    for (part, value, source) in slots {
        let consumable = match source {
            SlotSource::Skill { info, rank } => {
                // GetSkillInfoById(value).SkillID dereferences a missing SkillInfo.
                let active = info.ok_or(MissionError::Content)?;
                match rank {
                    Some(rank) if active => Consumable {
                        class: value,
                        bring: rank,
                        draft: 0,
                    },
                    _ => continue,
                }
            }
            SlotSource::Item { info: None, .. } => continue,
            SlotSource::Item {
                info: Some(info),
                usable,
            } => {
                if info.alchemist_box {
                    return Err(MissionError::Unsupported);
                }
                let multiplier = if info.sub_weapon {
                    sub_weapon_multiplier
                } else {
                    quick_multiplier
                };
                let cap = f64::from(info.quick_slot_max_stack as f32 * multiplier).ceil() as i32;
                let entry = match loaded.iter().position(|(c, _)| *c == info.class) {
                    Some(i) => i,
                    None => {
                        loaded.push((info.class.clone(), 0));
                        loaded.len() - 1
                    }
                };
                let already = loaded[entry].1;
                let bring = if usable > already {
                    (usable - already).min(cap)
                } else {
                    0
                };
                loaded[entry].1 += bring;
                Consumable {
                    class: info.class,
                    bring,
                    draft: info.draft,
                }
            }
        };
        match made.iter_mut().find(|(k, _)| *k == part) {
            Some(row) => row.1 = consumable,
            None => made.push((part, consumable)),
        }
    }
    let mut inventory = NetDictionary::default();
    for (part, c) in made {
        let limited = item_limit.is_some_and(|r| r.is_match(&c.class));
        let c = if limited || c.bring == 0 {
            Consumable {
                class: String::new(),
                bring: 0,
                draft: 0,
            }
        } else {
            c
        };
        // SetConsumable: remove the entry of the same class, then the part.
        if let Some(key) = inventory.key_of(&c.class) {
            inventory.remove(key);
        }
        inventory.remove(part);
        inventory.insert(part, c);
    }
    Ok(inventory.rows())
}

/// Dictionary<int, ConsumablesInfo> enumeration order: removed entries go to a LIFO
/// free list that the next insertion reuses.
#[derive(Default)]
struct NetDictionary {
    entries: Vec<Option<(i32, Consumable)>>,
    free: Vec<usize>,
}
impl NetDictionary {
    fn key_of(&self, class: &str) -> Option<i32> {
        self.entries
            .iter()
            .flatten()
            .find(|(_, c)| c.class == class)
            .map(|(k, _)| *k)
    }
    fn remove(&mut self, key: i32) {
        if let Some(i) = self
            .entries
            .iter()
            .position(|e| e.as_ref().is_some_and(|(k, _)| *k == key))
        {
            self.entries[i] = None;
            self.free.push(i);
        }
    }
    fn insert(&mut self, key: i32, value: Consumable) {
        match self.free.pop() {
            Some(i) => self.entries[i] = Some((key, value)),
            None => self.entries.push(Some((key, value))),
        }
    }
    fn rows(self) -> Vec<(i32, Consumable)> {
        self.entries.into_iter().flatten().collect()
    }
}

/// Outcome of a MonsterKilled report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kill {
    /// No current sector (killed on the ship).
    NotInSector,
    /// Neither a Monster nor its spawner ("is that client monster?").
    Unknown,
    /// The spawner exists but its Monster is gone ("monster already dead").
    AlreadyDead,
    Killed {
        monster_type: String,
        is_boss: bool,
        is_named: bool,
        /// MonsterSpawner.MonsterSpawnCount (Spawn runs once per sequence).
        spawn_count: i32,
        all_dead: bool,
        /// Monster.MonsterEntityName (the EvilCore key).
        key: String,
        /// Monster.DropItem at the kill.
        drop_item: i32,
        info: Option<MonsterInfo>,
    },
}

/// MonsterKillExpCalculator.GiveExp for one player of a solo play (weight ratio 1 and
/// PartyBonusRate.GetExpRatio(1) = 140), then MicroPlayer.GiveMonsterKillExp's
/// ExpRate_Monster. Single-precision products as the JIT computes them on x64.
pub fn monster_kill_exp(info: MonsterInfo, spawn_count: i32, exp_rate_monster: i32) -> i32 {
    let num = 0.max(100 - 20 * (spawn_count - 1));
    let base = (info.exp.wrapping_mul(num) / 100) as f32;
    let ratio = 1.0_f32;
    let exp = (f64::from(base * ratio * 140.0_f32) / 100.0) as i32;
    exp.wrapping_mul(exp_rate_monster) / 100
}

/// EvilCoreType values.
pub const EVIL_CORE: i32 = 1;
pub const BASIC_GOLD_CORE: i32 = 2;

/// ItemClassExBuilder.Parse: the class before ',', '[' or ']'.
pub fn item_class(class_ex: &str) -> &str {
    class_ex.split([',', '[', ']']).next().unwrap_or("")
}
