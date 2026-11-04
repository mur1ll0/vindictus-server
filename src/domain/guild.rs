//! In-game guild rules of the HeroesGuildAPI branch (FeatureMatrix InGameGuild and
//! GuildHeroesCore_v1 on in EN-EU): GuildAPI._CheckGroupName, HeroesGuildAPI.CheckGroupID,
//! the OpenGuildProcessor result strings, codeGroupUserType ↔ GuildMemberRank
//! (GuildAPI_Extension) and HeroesDataContext.GetMaxMemberLimit.
use crate::protocol::guild::rank;

/// FeatureMatrix GuildNamingRuleMaxBytes (EN-EU).
pub const NAME_MAX: usize = 20;
/// FeatureMatrix InGameGuild_MaxMember.
pub const DEFAULT_MAX_MEMBER: i32 = 100;
/// GuildCreate/GuildCheckGroupID @guildId nvarchar(24): longer IDs are truncated.
pub const ID_MAX: usize = 24;
/// GuildCreate @guildIntro nvarchar(200).
pub const INTRO_MAX: usize = 200;
/// UpdateGuildInfo @Notice nvarchar(256).
pub const NOTICE_MAX: usize = 256;
/// The story line that leads to the guild creation dialog (GuildStoryRequest, and deleted
/// once the guild is open).
pub const CREATE_STORY: &str = "Guild_create";

/// dbo.GuildUser.codeGroupUserType.
pub mod user_type {
    pub const MASTER: i16 = 5;
    pub const OPERATOR: i16 = 15;
    pub const MEMBER: i16 = 125;
    pub const WAIT: i16 = 155;
    /// GuildMemberRank.Unknown in the reverse conversion.
    pub const UNKNOWN: i16 = 235;
}

/// GuildAPI_Extension.ToGuildMemberRank.
pub const fn rank_of(user_type: i16) -> i32 {
    match user_type {
        user_type::MASTER => rank::MASTER,
        user_type::OPERATOR => rank::OPERATOR,
        user_type::MEMBER => rank::MEMBER,
        user_type::WAIT => rank::WAIT,
        _ => rank::IGNORABLE,
    }
}

/// GuildAPI_Extension.ToGroupUserType.
pub const fn user_type_of(member_rank: i32) -> i16 {
    match member_rank {
        rank::MASTER => user_type::MASTER,
        rank::OPERATOR => user_type::OPERATOR,
        rank::MEMBER => user_type::MEMBER,
        rank::WAIT => user_type::WAIT,
        _ => user_type::UNKNOWN,
    }
}

/// GuildMemberRank.IsRegularMember: Master, Operator or Member.
pub const fn is_regular(member_rank: i32) -> bool {
    matches!(member_rank, rank::MASTER | rank::OPERATOR | rank::MEMBER)
}

/// GetMaxMemberLimit: the stored limit when above InGameGuild_MaxMember, else that default
/// (GuildCreate stores 0).
pub const fn max_member_limit(stored: i32) -> i32 {
    if stored > DEFAULT_MAX_MEMBER {
        stored
    } else {
        DEFAULT_MAX_MEMBER
    }
}

/// The first `max` UTF-16 units of `text` (an nvarchar(n) parameter).
pub fn truncate_utf16(text: &str, max: usize) -> String {
    let mut units = 0;
    let mut out = String::new();
    for c in text.chars() {
        units += c.len_utf16();
        if units > max {
            break;
        }
        out.push(c);
    }
    out
}

/// GuildNamingRule `^[a-zA-Z]{2,}_?[a-zA-Z]{2,}$`.
fn matches_naming_rule(name: &str) -> bool {
    let letters = |s: &str| s.len() >= 2 && s.bytes().all(|b| b.is_ascii_alphabetic());
    match name.split_once('_') {
        Some((a, b)) => letters(a) && letters(b),
        None => name.len() >= 4 && letters(name),
    }
}

/// GuildAPI._CheckGroupName (repetition check off: NamingRuleRepeatingCharCheckCount -1),
/// as the OpenGuild result string. The forbidden-word list is empty in EN-EU
/// (GuildForbiddenWords is absent from its content database).
pub fn check_name(name: &str) -> Result<(), &'static str> {
    let simple = name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    let length = name.encode_utf16().count();
    if (simple && length > NAME_MAX) || (!simple && length > NAME_MAX / 2) {
        return Err(open_error::NAMING_RULE_MAX_BYTES);
    }
    if !matches_naming_rule(name) {
        return Err(open_error::NAMING_RULE);
    }
    Ok(())
}

/// HeroesGuildAPI.CheckGroupID before the duplicate lookup.
pub fn check_id(id: &str) -> Result<(), &'static str> {
    if id.is_empty() {
        return Err(open_error::EMPTY_ID);
    }
    Ok(())
}

/// OpenGuildProcessor result strings (GuildResultMessage.Arg of OpenGuildFail).
pub mod open_error {
    /// GuildCreate returned -1, or a forbidden word in the ID.
    pub const ID: &str = "GuildID";
    pub const NAMING_RULE: &str = "GuildNamingRule";
    pub const DUPLICATED_NAME: &str = "GuildDuplicatedName";
    pub const NAMING_RULE_MAX_BYTES: &str = "GuildNamingRuleMaxBytes";
    /// GuildCreate returned -6: the character already has a GuildUser row.
    pub const REPEATED_CHARACTERS: &str = "GuildRepeatedCharacters";
    pub const EMPTY_ID: &str = "GuildEmptyID";
    pub const DUPLICATED_ID: &str = "GuildDuplicatedID";
    /// The FE state answers of a member: OpenGuildFail/JoinGuildFail "HasGuild".
    pub const HAS_GUILD: &str = "HasGuild";
    /// CloseGuildFail in OutGuild.
    pub const NO_GUILD: &str = "NoGuild";
}

/// QueryGuildListProcessor: a search key of 2 to GuildNamingRuleMaxBytes characters.
pub fn search_key_valid(key: &str) -> bool {
    (2..=NAME_MAX).contains(&key.encode_utf16().count())
}

/// GuildGroupGetListByGuildName: the key without the characters the procedure strips.
pub fn search_name_key(key: &str) -> String {
    key.chars()
        .filter(|c| !matches!(c, '!' | '(' | ')' | '[' | ']' | '"' | '@' | ',' | ' '))
        .collect()
}

/// QueryGuildListProcessor total page: 1 without rows, else ceil(rows / page size); None
/// when the page size is 0 (the source divides by zero and the operation fails).
pub fn total_pages(rows: i64, page_size: u8) -> Option<i32> {
    if rows <= 0 {
        return Some(1);
    }
    let size = i64::from(page_size);
    if size == 0 {
        return None;
    }
    i32::try_from((rows - 1) / size + 1).ok()
}

/// GuildMemberInfo.LastLoginTime: (UtcNow − last connect).Days, from SQL datetime units of
/// 1/300 s.
pub fn days_since(now_units300: i64, last_units300: i64) -> i32 {
    let days = (now_units300 - last_units300) / (300 * 86_400);
    i32::try_from(days).unwrap_or(i32::MAX)
}

/// GPGainType.
pub mod gain {
    pub const BY_PVE: u8 = 1;
    pub const BY_PVP: u8 = 2;
    pub const BY_CHEAT: u8 = 255;
}

/// GPGainTypeUtil.IsValidGPGainTypeValue.
pub const fn valid_gain(value: u8, include_cheat: bool) -> bool {
    (include_cheat && value == gain::BY_CHEAT) || (value >= 1 && value < 3)
}

/// GuildContents.LoadGuildDailyGPLimit + GetDailyGPLimit: the `;`-separated limits of
/// FeatureMatrix GuildLevel_DailyGPLimit for PvE then PvP, each ×10 (−1 is unlimited, an
/// unparsable value 0); a type without a value is unlimited; an invalid type gets 0.
pub fn daily_limit(spec: &str, gain_type: u8) -> i32 {
    if !valid_gain(gain_type, true) {
        return 0;
    }
    if spec.is_empty() {
        return -1;
    }
    let index = usize::from(gain_type);
    match spec
        .split(';')
        .nth(index.wrapping_sub(1))
        .filter(|_| index < 3)
    {
        None => -1,
        Some(v) => match v.parse::<i32>() {
            Ok(-1) => -1,
            Ok(n) => n.wrapping_mul(10),
            Err(_) => 0,
        },
    }
}

/// GuildContents.GetPrevDailyGPResetTime in Unix seconds: today's reset (UTC midnight plus
/// GuildLevel_DailyGPResetTimeAdder hours; 0 when that is a day or more, and a negative adder
/// stays negative as the source discards its TimeSpan.Add), or yesterday's when it is ahead.
pub fn prev_reset(now: i64, adder_hours: i32) -> i64 {
    let adder = if adder_hours.abs() >= 24 {
        0
    } else {
        i64::from(adder_hours) * 3600
    };
    let reset = now.div_euclid(86_400) * 86_400 + adder;
    if reset > now { reset - 86_400 } else { reset }
}

/// GuildContents.GuildLevelUpInfoDic: the enabled rows by level with RequiredExp × 10 and
/// the level-up reward item class.
#[derive(Clone, Debug, Default)]
pub struct Levels {
    rows: std::collections::BTreeMap<i32, (i64, Option<String>)>,
}
impl Levels {
    /// The enabled rows; a repeated level fails as ToDictionary does.
    pub fn new(
        rows: impl IntoIterator<Item = (i32, i64, Option<String>)>,
    ) -> Result<Self, &'static str> {
        let mut out = std::collections::BTreeMap::new();
        for (level, exp, reward) in rows {
            if out.insert(level, (exp.wrapping_mul(10), reward)).is_some() {
                return Err("repeated GuildLevelUpInfo level");
            }
        }
        Ok(Self { rows: out })
    }
    /// GuildLevelUpInfoDic.Keys.Max() + 1 (None without rows).
    pub fn max_level(&self) -> Option<i32> {
        self.rows.keys().next_back().map(|l| l.wrapping_add(1))
    }
    pub fn required(&self, level: i32) -> Option<i64> {
        self.rows.get(&level).map(|(exp, _)| *exp)
    }
    /// GetGuildLevelUpRewardItemClass: None for a level without a row or with no class.
    pub fn reward(&self, level: i32) -> Option<&str> {
        self.rows
            .get(&level)
            .and_then(|(_, r)| r.as_deref())
            .filter(|r| !r.is_empty())
    }
}

/// What GuildEntity.GiveGP did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gain {
    /// The GP given (after the daily limit).
    pub given: i32,
    /// The new daily gain of the type, with the GuildGainGP push (valid gain types only).
    pub daily: Option<i32>,
    /// GuildPoint before the gain (GuildGainGPMessage.GuildPoint).
    pub point_before: i64,
    pub level: i32,
    pub point: i64,
    /// The level reached by each level up, in order.
    pub level_ups: Vec<i32>,
}

/// GuildEntity.GiveGP on the guild's level and point with today's gain of the type
/// (`daily`, None when the type has no entry yet). None when nothing is given.
pub fn give_gp(
    levels: &Levels,
    level: i32,
    point: i64,
    daily: Option<i32>,
    limit: i32,
    gp: i32,
    gain_type: u8,
) -> Option<Gain> {
    if gp == 0 {
        return None;
    }
    let max = levels.max_level()?;
    if level >= max {
        return None;
    }
    let mut required = levels.required(level)?;
    let mut gp = gp;
    let mut new_daily = None;
    if valid_gain(gain_type, false) {
        let current = daily.unwrap_or(0);
        if gp > 0 {
            if limit >= 0 {
                if current >= limit {
                    return None;
                }
                if current.wrapping_add(gp) > limit {
                    gp = limit.wrapping_sub(current);
                    if gp < 0 {
                        return None;
                    }
                }
            }
        } else {
            if current.wrapping_add(gp) < 0 {
                gp = current.wrapping_neg();
            }
            if gp == 0 {
                return None;
            }
        }
        new_daily = Some(current.wrapping_add(gp));
    }
    let point_before = point;
    let mut level = level;
    let mut point = point;
    let mut level_ups = Vec::new();
    let mut rest = point.wrapping_add(i64::from(gp)).wrapping_sub(required);
    if rest >= 0 {
        while rest >= 0 {
            level = level.wrapping_add(1);
            point = rest;
            level_ups.push(level);
            if level >= max {
                point = 0;
                break;
            }
            let Some(next) = levels.required(level) else {
                break;
            };
            required = next;
            rest = point.wrapping_sub(required);
        }
    } else {
        point = point.wrapping_add(i64::from(gp));
    }
    Some(Gain {
        given: gp,
        daily: new_daily,
        point_before,
        level,
        point,
        level_ups,
    })
}
