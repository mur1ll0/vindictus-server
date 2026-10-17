//! Pinned, private first-Fiona projection. No original values implement a wire default.
use crate::{
    domain::{
        character_base_stats::BaseStats,
        feature_policy::FeaturePolicy,
        legacy_user_data::{LegacyBytes, SqlDateTime2},
    },
    error::Error,
    infrastructure::{
        item_catalog::Package as Catalog,
        user_export::{self, PrivateText, nullable},
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};
use zeroize::Zeroizing;
fn fail() -> Error {
    Error::FionaInitializerProfile
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feature {
    key: String,
    #[serde(deserialize_with = "nullable")]
    value: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Level {
    level: i32,
    required_exp: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preset {
    title_id: i32,
    class_restriction: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Title {
    title_id: i32,
    #[serde(deserialize_with = "nullable")]
    auto_give_level: Option<i32>,
    class_restriction: i32,
    #[serde(deserialize_with = "nullable")]
    feature: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Constraint {
    #[serde(rename = "id_utf16")]
    _id_utf16: PrivateText,
    #[serde(rename = "rank")]
    _rank: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Next {
    required_ap: i32,
    require_unlock: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Skill {
    id_utf16: PrivateText,
    rank: i32,
    class_restriction: i32,
    use_type: i32,
    level_constraint: i32,
    #[serde(deserialize_with = "nullable")]
    rank_feature: Option<String>,
    constraints: Vec<Constraint>,
    #[serde(deserialize_with = "nullable")]
    next_rank: Option<Next>,
    status_effect_types: [Option<String>; 3],
    item_callback: bool,
    stat_callback: bool,
    meditation: bool,
    argument_rows: usize,
    argument_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Attendance {
    version: i32,
    date_from_ticks: PrivateText,
    date_until_ticks: PrivateText,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Export {
    format_version: u32,
    source: String,
    database_sha256: String,
    config_sha256: String,
    state_sha256: String,
    implementations: BTreeMap<String, String>,
    features: Vec<Feature>,
    enabled_keys: Vec<String>,
    base_stats: [i32; 6],
    levels: Vec<Level>,
    presets: Vec<Preset>,
    title_info: Vec<Title>,
    skills: Vec<Skill>,
    attendance: Vec<Attendance>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    current: String,
    value: String,
    enabled: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    format_version: u32,
    source: String,
    profile_sha256: String,
    database_sha256: String,
    config_sha256: String,
    feature_matrix_sha256: String,
    profiles: Vec<String>,
    pairs: Vec<Pair>,
}
#[derive(Deserialize)]
struct CodeSource {
    path: String,
    sha256: String,
}
struct AttendancePeriod {
    from: i64,
    until: i64,
}
/// These names/arguments are private content, not diagnostic output.
pub struct InitialSkill {
    key: LegacyBytes<50, true>,
    rank: i32,
    required_ap: i32,
    item_callback: bool,
    stat_callback: bool,
}
impl InitialSkill {
    pub fn key(&self) -> &[u8] {
        self.key.as_bytes()
    }
    pub fn rank(&self) -> i32 {
        self.rank
    }
    pub fn required_ap(&self) -> i32 {
        self.required_ap
    }
    pub fn item_callback(&self) -> bool {
        self.item_callback
    }
    pub fn stat_callback(&self) -> bool {
        self.stat_callback
    }
}
pub struct Profile {
    db: String,
    config: String,
    state: [u8; 32],
    basis: [i32; 6],
    level_exp: i32,
    presets: Vec<i32>,
    skills: Vec<InitialSkill>,
    policy: FeaturePolicy,
    attendance: BTreeMap<i32, AttendancePeriod>,
}
fn bytes(reader: impl Read, bound: usize) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut b = Zeroizing::new(Vec::new());
    reader
        .take(bound as u64 + 1)
        .read_to_end(&mut b)
        .map_err(|_| fail())?;
    if b.len() > bound {
        return Err(fail());
    };
    Ok(b)
}
fn hash(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn hash_bytes(text: String) -> Result<[u8; 32], Error> {
    user_export::hex(PrivateText(text), 32)?
        .try_into()
        .map_err(|_| fail())
}
fn ticks(v: PrivateText) -> Result<i64, Error> {
    Ok(SqlDateTime2::from_ticks(user_export::integer(v)?)?.ticks())
}
impl Profile {
    pub fn read(input: impl Read, reference: impl Read) -> Result<Self, Error> {
        let data = bytes(input, 16 * 1024 * 1024)?;
        let evidence = bytes(reference, 256 * 1024)?;
        let r: Export = serde_json::from_slice(&data).map_err(|_| fail())?;
        let reference: Reference = serde_json::from_slice(&evidence).map_err(|_| fail())?;
        let known: BTreeMap<String, CodeSource> =
            serde_json::from_str(include_str!("../domain/fiona_initializer_sources.json"))
                .map_err(|_| fail())?;
        let feature_source = known.get("feature_matrix").ok_or_else(fail)?;
        if r.format_version != 1
            || r.source != "heroesContents.first-fiona-initializer"
            || !hash(&r.database_sha256)
            || !hash(&r.config_sha256)
            || !hash(&r.state_sha256)
            || r.implementations.len() != known.len() - 1
            || known.iter().any(|(key, v)| {
                !hash(&v.sha256)
                    || !v.path.starts_with("legacy/decompiled/")
                    || (key != "feature_matrix" && r.implementations.get(key) != Some(&v.sha256))
            })
            || reference.format_version != 1
            || reference.source != "Framework.FeatureMatrix.IsEnable"
            || reference.profile_sha256 != format!("{:x}", Sha256::digest(data.as_slice()))
            || reference.database_sha256 != r.database_sha256
            || reference.config_sha256 != r.config_sha256
            || reference.feature_matrix_sha256 != feature_source.sha256
            || reference.profiles != ["Framework48-x86-en-US", "Framework48-x64-en-US"]
            || reference.pairs.len() > 128
            || r.features.len() > 128
            || r.enabled_keys.len() > 128
            || r.levels.len() > 1
            || r.presets.len() > 4096
            || r.title_info.len() > 4096
            || r.skills.len() > 256
            || r.attendance.len() > 4096
        {
            return Err(fail());
        }
        let mut enabled = BTreeSet::new();
        if r.enabled_keys
            .iter()
            .any(|k| k.is_empty() || k.len() > 512 || !k.is_ascii() || !enabled.insert(k.as_str()))
        {
            return Err(fail());
        }
        let mut feature_keys = BTreeSet::new();
        if r.features
            .iter()
            .any(|f| !feature_keys.insert(f.key.as_str()))
        {
            return Err(fail());
        }
        if !enabled.iter().all(|k| feature_keys.contains(k)) {
            return Err(fail());
        }
        let policy = FeaturePolicy::with_recorded_comparisons(
            r.features.into_iter().map(|f| (f.key, f.value)),
            reference
                .pairs
                .into_iter()
                .map(|p| (p.current, p.value, p.enabled)),
        )?;
        for (key, value) in [
            ("AutoItemLogging", true),
            ("ProcessAfterDeleteItem", false),
            ("InGameGuild", true),
            ("GuildScore", false),
            ("RandomMission", false),
            ("AddTitleFromPreset", true),
            ("DisableCharacterCreation", false),
            ("ServerTrouble", false),
            ("DisableLogging", false),
            ("DisableOperationLogging", false),
        ] {
            if !enabled.contains(key) || policy.is_enabled(Some(key))? != value {
                return Err(fail());
            }
        }
        if !enabled.contains("SharedInventory") {
            return Err(fail());
        }
        if !policy
            .get_string("Event_newbie_character_preset_item")
            .is_empty()
        {
            return Err(fail());
        }
        let level_exp = match r.levels.as_slice() {
            [] => 0,
            [l] if l.level == 1 => {
                if policy.is_enabled(l.feature.as_deref())? {
                    l.required_exp
                } else {
                    0
                }
            }
            _ => return Err(fail()),
        };
        let mut initial = BTreeMap::new();
        for p in r.presets {
            if initial.insert(p.title_id, p.class_restriction).is_some() {
                return Err(fail());
            }
        }
        let mut titles = BTreeMap::new();
        for t in r.title_info {
            let id = t.title_id;
            if titles.insert(id, t).is_some() {
                return Err(fail());
            }
        }
        let mut effective = BTreeMap::new();
        for (id, class) in &initial {
            if let Some(t) = titles.get(id)
                && t.auto_give_level == Some(1)
                && policy.is_enabled(t.feature.as_deref())?
            {
                effective.insert(*id, *class);
            }
        }
        for (id, t) in &titles {
            if t.auto_give_level == Some(1)
                && policy.is_enabled(t.feature.as_deref())?
                && !initial.contains_key(id)
            {
                effective.insert(*id, t.class_restriction);
            }
        }
        // This minimum admits the measured already-synchronized source; no silent preset repair.
        if effective != initial {
            return Err(fail());
        }
        let presets = effective
            .into_iter()
            .filter_map(|(id, class)| ((class & 2) != 0).then_some(id))
            .collect();
        let mut seen = BTreeSet::new();
        let mut skills = Vec::new();
        for s in r.skills {
            let name = LegacyBytes::<50, true>::from_bytes(user_export::hex(s.id_utf16, 100)?)?;
            if name.as_bytes().is_empty()
                || !seen.insert(name.as_bytes().to_vec())
                || s.rank.checked_add(1).is_none()
                || s.use_type != 0
                || (s.class_restriction & 2) == 0
                || s.level_constraint > 1
                || !policy.is_enabled(s.rank_feature.as_deref())?
                || !s.constraints.is_empty()
                || s.status_effect_types.iter().any(Option::is_some)
                || s.meditation
                || s.argument_rows > 4096
                || !hash(&s.argument_sha256)
                || s.next_rank.as_ref().is_some_and(|n| n.require_unlock)
            {
                return Err(fail());
            }
            let required_ap = s
                .next_rank
                .as_ref()
                .filter(|n| n.required_ap > 0)
                .map_or(-1, |n| n.required_ap);
            skills.push(InitialSkill {
                key: name,
                rank: s.rank,
                required_ap,
                item_callback: s.item_callback,
                stat_callback: s.stat_callback,
            });
        }
        let mut attendance = BTreeMap::new();
        for a in r.attendance {
            let period = AttendancePeriod {
                from: ticks(a.date_from_ticks)?,
                until: ticks(a.date_until_ticks)?,
            };
            if period.from > period.until || attendance.insert(a.version, period).is_some() {
                return Err(fail());
            }
        }
        Ok(Self {
            db: r.database_sha256,
            config: r.config_sha256,
            state: hash_bytes(r.state_sha256)?,
            basis: r.base_stats,
            level_exp,
            presets,
            skills,
            policy,
            attendance,
        })
    }
    pub fn bind(&self, catalog: &Catalog) -> Result<(), Error> {
        if self.db != catalog.manifest.database_sha256
            || self.config != catalog.manifest.config_sha256
        {
            return Err(fail());
        };
        Ok(())
    }
    pub fn source_state(&self) -> &[u8; 32] {
        &self.state
    }
    pub fn base_stats(&self, title_contribution: [i32; 7]) -> BaseStats {
        BaseStats::compose(self.basis, title_contribution)
    }
    pub fn level_up_exp(&self) -> i32 {
        self.level_exp
    }
    pub fn preset_ids(&self) -> &[i32] {
        &self.presets
    }
    pub fn skills(&self) -> &[InitialSkill] {
        &self.skills
    }
    pub fn policy(&self) -> &FeaturePolicy {
        &self.policy
    }
    /// A currently active newbie registration needs the full attendance writer, not a no-op.
    pub fn validate_attendance_window(&self, now: SqlDateTime2) -> Result<(), Error> {
        if self.policy.is_enabled(Some("Attendance_Newbie"))? {
            let version = self.policy.get_integer("AttendanceEvent_Newbie")?;
            if version > 0
                && let Some(p) = self.attendance.get(&version)
                && p.from <= now.ticks()
                && now.ticks() <= p.until
            {
                return Err(fail());
            }
        }
        Ok(())
    }
}
