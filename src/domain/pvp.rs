//! PvpService contents (B5 PV2): PvpConfig.Load over heroesContents PvpInfo rows (PvpMode,
//! Key, Value, Feature). A row counts when FeatureMatrix.IsEnable(Feature) (empty is enabled);
//! the rows of a mode are its values and GameRule picks the `<GameRule>Config` class, whose
//! [PvpConfigValue] properties take the row of their name or the default. ClientConfig is
//! PvpMode, GameRule and every client property in declaration order, with the raw text.
//! Only the rules of the EN-EU modes are migrated (Arena, DeathMatch, GodMatch); a mode of
//! another rule (CTF, Survivor, FruitMatch, all P2P modes off in EN-EU) is left out.
//! PvpPresetInfo has rows only for the p01 and p06 maps, none of these modes, so the presets
//! change nothing here.
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameRule {
    Arena,
    DeathMatch,
    GodMatch,
}
impl GameRule {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "Arena" => Some(Self::Arena),
            "DeathMatch" => Some(Self::DeathMatch),
            "GodMatch" => Some(Self::GodMatch),
            _ => None,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Arena => "Arena",
            Self::DeathMatch => "DeathMatch",
            Self::GodMatch => "GodMatch",
        }
    }
}

/// PvpService.RegisterMatchingSystem: the MatchingSystem value, lower case, split on ':'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Matching {
    Partial,
    Full,
    Arena,
    /// statbase[:seconds], 30 by default.
    StatBase(u32),
}
impl Matching {
    pub fn parse(text: &str) -> Option<Self> {
        let lower = text.to_lowercase();
        let mut parts = lower.split(':');
        match parts.next()? {
            "partial" => Some(Self::Partial),
            "full" => Some(Self::Full),
            "arena" => Some(Self::Arena),
            "statbase" => Some(Self::StatBase(match parts.next() {
                Some(s) => s.trim().parse().ok()?,
                None => 30,
            })),
            _ => None,
        }
    }
}

/// One [PvpConfigValue] property: name, default, IsClientConfig.
type Property = (&'static str, &'static str, bool);

/// ArenaConfig in declaration order.
const ARENA: [Property; 20] = [
    ("GameLength", "180", true),
    ("BSP", "p05", true),
    ("MaxSize", "30", true),
    ("TeamSize", "1", true),
    ("WaitBeforeStart", "120", true),
    ("WaitBetweenRound", "15", true),
    ("RoundToWin", "1", true),
    ("PairOfPlayer", "4", true),
    ("MatchingSystem", "Arena", false),
    ("LevelLimitMin", "-1", false),
    ("LevelLimitMax", "-1", false),
    ("PvpUserCountKey", "PVP_Arena", false),
    ("IsGuild", "false", false),
    ("RewardItem", "match_stamp", false),
    ("WinSignet", "3", false),
    ("DrawSignet", "1", false),
    ("LoseSignet", "1", false),
    ("EventRewardItem", "", false),
    ("ArenaChannelCount", "3", false),
    ("SamePlayerRewardCoolDown", "2", false),
];
/// DeathMatchConfig in declaration order.
const DEATH_MATCH: [Property; 18] = [
    ("GameLength", "600", true),
    ("BSP", "p03", true),
    ("MaxSize", "16", false),
    ("TeamSize", "1", true),
    ("WaitBeforeStart", "60", true),
    ("WaitBetweenRound", "10", true),
    ("RoundToWin", "2", true),
    ("MatchingSystem", "Full", false),
    ("LevelLimitMin", "-1", false),
    ("LevelLimitMax", "-1", false),
    ("PvpUserCountKey", "PVP_DMatch", false),
    ("IsGuild", "true", false),
    ("RewardItem", "match_stamp", false),
    ("WinSignet", "3", false),
    ("DrawSignet", "1", false),
    ("LoseSignet", "1", false),
    ("EventRewardItem", "", false),
    ("MinClearTime", "300", false),
];
/// GodMatchConfig in declaration order.
const GOD_MATCH: [Property; 18] = [
    ("GameLength", "1200", true),
    ("BSP", "p04", true),
    ("MaxSize", "16", false),
    ("TeamSize", "8", true),
    ("WaitBeforeStart", "60", true),
    ("WaitBetweenRound", "10", true),
    ("RoundToWin", "1", true),
    ("MatchingSystem", "Full", false),
    ("LevelLimitMin", "-1", false),
    ("LevelLimitMax", "-1", false),
    ("PvpUserCountKey", "PVP_GMatch", false),
    ("IsGuild", "false", false),
    ("RewardItem", "match_stamp", false),
    ("WinSignet", "3", false),
    ("DrawSignet", "1", false),
    ("LoseSignet", "1", false),
    ("EventRewardItem", "", false),
    ("MinClearTime", "300", false),
];

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PvpContentsError {
    #[error("PvpInfo value does not parse")]
    Value,
    #[error("PvpInfo feature expression")]
    Feature,
}

/// A loaded mode (IPvpConfig).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PvpConfig {
    pub mode: String,
    pub rule: GameRule,
    pub game_length: i32,
    pub bsp: String,
    pub max_size: i32,
    pub team_size: i32,
    pub wait_before_start: i32,
    pub wait_between_round: i32,
    pub round_to_win: i32,
    /// Arena only (4 elsewhere, unused).
    pub pair_of_player: i32,
    pub matching: Matching,
    pub level_min: i32,
    pub level_max: i32,
    pub user_count_key: String,
    pub is_guild: bool,
    pub reward_item: String,
    pub win_signet: i32,
    pub draw_signet: i32,
    pub lose_signet: i32,
    pub event_reward_item: String,
    /// Arena only.
    pub arena_channel_count: i32,
    /// Arena only, minutes.
    pub same_player_reward_cool_down: i32,
    /// DeathMatch and GodMatch only, seconds.
    pub min_clear_time: i32,
    /// ClientConfig in insertion order.
    pub client_config: Vec<(String, String)>,
}

/// Int32.Parse with NumberStyles.Integer (whitespace and a sign).
fn int(text: &str) -> Result<i32, PvpContentsError> {
    text.trim().parse().map_err(|_| PvpContentsError::Value)
}
/// Boolean.Parse: "True"/"False" in any case, surrounding whitespace allowed.
fn boolean(text: &str) -> Result<bool, PvpContentsError> {
    match text.trim().to_lowercase().as_str() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(PvpContentsError::Value),
    }
}

impl PvpConfig {
    fn build(mode: &str, rule: GameRule, values: &BTreeMap<String, String>) -> Result<Self, PvpContentsError> {
        let properties: &[Property] = match rule {
            GameRule::Arena => &ARENA,
            GameRule::DeathMatch => &DEATH_MATCH,
            GameRule::GodMatch => &GOD_MATCH,
        };
        let mut client_config = vec![
            ("PvpMode".to_owned(), mode.to_owned()),
            ("GameRule".to_owned(), rule.name().to_owned()),
        ];
        let mut resolved: BTreeMap<&str, String> = BTreeMap::new();
        for (name, default, client) in properties {
            let text = values.get(*name).cloned().unwrap_or_else(|| (*default).to_owned());
            if *client {
                client_config.push(((*name).to_owned(), text.clone()));
            }
            resolved.insert(name, text);
        }
        let get = |name: &str, default: &str| -> String {
            resolved.get(name).cloned().unwrap_or_else(|| default.to_owned())
        };
        let n = |name: &str, default: &str| int(&get(name, default));
        Ok(Self {
            mode: mode.to_owned(),
            rule,
            game_length: n("GameLength", "0")?,
            bsp: get("BSP", ""),
            max_size: n("MaxSize", "0")?,
            team_size: n("TeamSize", "0")?,
            wait_before_start: n("WaitBeforeStart", "0")?,
            wait_between_round: n("WaitBetweenRound", "0")?,
            round_to_win: n("RoundToWin", "0")?,
            pair_of_player: n("PairOfPlayer", "4")?,
            matching: Matching::parse(&get("MatchingSystem", "")).ok_or(PvpContentsError::Value)?,
            level_min: n("LevelLimitMin", "-1")?,
            level_max: n("LevelLimitMax", "-1")?,
            user_count_key: get("PvpUserCountKey", ""),
            is_guild: boolean(&get("IsGuild", "false"))?,
            reward_item: get("RewardItem", ""),
            win_signet: n("WinSignet", "0")?,
            draw_signet: n("DrawSignet", "0")?,
            lose_signet: n("LoseSignet", "0")?,
            event_reward_item: get("EventRewardItem", ""),
            arena_channel_count: n("ArenaChannelCount", "0")?,
            same_player_reward_cool_down: n("SamePlayerRewardCoolDown", "0")?,
            min_clear_time: n("MinClearTime", "0")?,
            client_config,
        })
    }
}

/// The PvpInfo row: PvpMode, Key, Value, Feature.
pub type InfoRow = (String, String, Option<String>, Option<String>);

/// PvpConfig.ConfigDict.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PvpContents {
    configs: BTreeMap<String, PvpConfig>,
}
impl PvpContents {
    /// PvpConfig.Load: `enabled` is FeatureMatrix.IsEnable; a later enabled row of the same
    /// mode and key replaces an earlier one.
    pub fn load(
        rows: &[InfoRow],
        enabled: &mut dyn FnMut(Option<&str>) -> Result<bool, PvpContentsError>,
    ) -> Result<Self, PvpContentsError> {
        let mut modes: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (mode, key, value, feature) in rows {
            if enabled(feature.as_deref().filter(|f| !f.is_empty()))? {
                modes
                    .entry(mode.clone())
                    .or_default()
                    .insert(key.clone(), value.clone().unwrap_or_default());
            }
        }
        let mut configs = BTreeMap::new();
        for (mode, values) in &modes {
            let Some(rule) = values.get("GameRule").and_then(|r| GameRule::parse(r)) else {
                continue;
            };
            configs.insert(mode.clone(), PvpConfig::build(mode, rule, values)?);
        }
        Ok(Self { configs })
    }
    /// PvpConfig.Get.
    pub fn get(&self, mode: &str) -> Option<&PvpConfig> {
        self.configs.get(mode)
    }
    pub fn modes(&self) -> impl Iterator<Item = &PvpConfig> {
        self.configs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(mode: &str, key: &str, value: &str, feature: Option<&str>) -> InfoRow {
        (
            mode.to_owned(),
            key.to_owned(),
            Some(value.to_owned()),
            feature.map(str::to_owned),
        )
    }

    #[test]
    fn configs_follow_pvp_info_features_and_defaults() {
        let rows = vec![
            row("p05", "BSP", "p05", None),
            row("p05", "GameLength", "90", None),
            row("p05", "GameRule", "Arena", Some("Arena")),
            row("p05", "IsGuild", "FALSE", None),
            row("p05", "MatchingSystem", "Arena", None),
            row("p05", "MaxSize", "32", None),
            row("p05", "TeamSize", "1", None),
            row("p03", "GameRule", "DeathMatch", Some("DeathMatch")),
            row("p03", "MatchingSystem", "Full", Some("!PvpStatBaseMatching")),
            row("p03", "MatchingSystem", "statbase:60", Some("PvpStatBaseMatching")),
            row("p03", "TeamSize", "8", None),
            row("p01", "GameRule", "CTF", None),
            row("p03_dev", "GameRule", "DeathMatch", Some("DevTest")),
        ];
        let on = ["Arena", "DeathMatch", "!PvpStatBaseMatching"];
        let c = PvpContents::load(&rows, &mut |f| Ok(f.is_none_or(|f| on.contains(&f)))).unwrap();
        let arena = c.get("p05").unwrap();
        assert_eq!((arena.max_size, arena.team_size, arena.game_length), (32, 1, 90));
        assert_eq!((arena.arena_channel_count, arena.win_signet, arena.is_guild), (3, 3, false));
        assert_eq!(arena.matching, Matching::Arena);
        assert_eq!(
            arena.client_config.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
            [
                "PvpMode", "GameRule", "GameLength", "BSP", "MaxSize", "TeamSize",
                "WaitBeforeStart", "WaitBetweenRound", "RoundToWin", "PairOfPlayer"
            ]
        );
        assert_eq!(arena.client_config[2], ("GameLength".to_owned(), "90".to_owned()));
        let dm = c.get("p03").unwrap();
        assert_eq!((dm.matching, dm.team_size, dm.is_guild), (Matching::Full, 8, true));
        assert_eq!(dm.client_config.len(), 8);
        // CTF is not migrated; a disabled GameRule row drops the mode.
        assert!(c.get("p01").is_none() && c.get("p03_dev").is_none());
        assert_eq!(Matching::parse("statbase"), Some(Matching::StatBase(30)));
        assert_eq!(Matching::parse("StatBase:60"), Some(Matching::StatBase(60)));
        assert_eq!(Matching::parse("dsonly"), None);
    }
}
