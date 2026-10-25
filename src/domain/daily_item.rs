//! DailyItemProcedure and TimeRange of the DailyItemController: a DailyItemProcessInfo
//! row runs once per processing range (its Period, or the whole DateFrom..DateUntil) for a
//! character whose CafeLevel/CafeType and level fit; times are UTC .NET ticks.

/// DailyItemProcedure.ProcessType (Enum.Parse of the first QueryString word, ignoring case).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProcessKind {
    GiveItem,
    TakeItem,
    GiveItemUntil,
    TakeItemUntil,
    GiveItemUntilByCharacterClass,
    /// GiveFatigueItem, guild and rank-title processes, GiveBaseHair, ExtendExpireTime.
    NotMigrated(String),
}

/// SQLDateTime.MinValue (1753-01-01) in ticks: an unprocessed procedure.
pub const SQL_MIN_TICKS: i64 = 552_877_920_000_000_000;
const TICKS_PER_MINUTE: i64 = 600_000_000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Procedure {
    pub id: i32,
    pub start: i64,
    pub end: i64,
    pub period: Option<i64>,
    pub cafe_level: i32,
    pub cafe_type: i32,
    pub level_min: i32,
    pub level_max: i32,
    pub kind: ProcessKind,
    pub args: Vec<String>,
    pub message: Option<String>,
}

impl Procedure {
    /// DailyItemProcedure.FromDailyItemProcessInfo: None for an unknown ProcessType.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        id: i32,
        start: i64,
        end: i64,
        period_minutes: i32,
        cafe_level: i32,
        cafe_type: i32,
        level_min: Option<i32>,
        level_max: Option<i32>,
        query: &str,
        message: Option<String>,
    ) -> Option<Self> {
        let words: Vec<&str> = query.split(' ').collect();
        let first = words.first().copied().unwrap_or("");
        let kind = match first.to_ascii_lowercase().as_str() {
            "giveitem" => ProcessKind::GiveItem,
            "takeitem" => ProcessKind::TakeItem,
            "giveitemuntil" => ProcessKind::GiveItemUntil,
            "takeitemuntil" => ProcessKind::TakeItemUntil,
            "giveitemuntilbycharacterclass" => ProcessKind::GiveItemUntilByCharacterClass,
            other @ ("givefatigueitem"
            | "givefatigueitem_event"
            | "updatecharacterranktitle"
            | "updateranktitle"
            | "giveitembyguildlevel"
            | "givebasehair"
            | "extendexpiretime") => ProcessKind::NotMigrated(other.to_owned()),
            _ => return None,
        };
        // The arguments are the words other than the first (any word equal to it is dropped).
        let args = words
            .iter()
            .filter(|w| **w != first)
            .map(|w| (*w).to_owned())
            .collect();
        Some(Self {
            id,
            start,
            end,
            period: (period_minutes > 0).then(|| i64::from(period_minutes) * TICKS_PER_MINUTE),
            cafe_level,
            cafe_type,
            level_min: level_min.unwrap_or(0),
            level_max: level_max.unwrap_or(i32::MAX),
            kind,
            args,
            message,
        })
    }

    /// TimeRange.GetCurrentPeriod: (start, end) of the range holding `now`.
    pub fn current_range(&self, now: i64) -> (i64, i64) {
        let Some(period) = self.period else {
            return (self.start, self.end);
        };
        // Ticks / Period.Ticks truncates toward zero, like the source.
        let start = self.start + (now - self.start) / period * period;
        let end = if self.end - period < start {
            self.end
        } else {
            start + period
        };
        (start, end)
    }

    /// DailyItemProcedure.Process admission: not yet processed in the current range and
    /// `now` strictly inside DateFrom..DateUntil.
    pub fn is_due(&self, processed: Option<i64>, now: i64) -> bool {
        let (start, end) = self.current_range(now);
        let processed = processed.unwrap_or(SQL_MIN_TICKS);
        !(start <= processed && processed < end) && self.start < now && now < self.end
    }

    /// The start of the next range after `now` (DailyItemScheduler.Enqueue of GetNextRange).
    pub fn next_range_start(&self, now: i64) -> Option<i64> {
        self.period?;
        let (_, end) = self.current_range(now);
        (end < self.end && end > now).then_some(end)
    }

    /// CheckConstraint: the character level within LevelMin..LevelMax.
    pub fn fits_level(&self, level: i32) -> bool {
        self.level_min <= level && level <= self.level_max
    }

    /// The argument count (one when absent).
    pub fn count(&self) -> Option<i32> {
        match self.args.get(1) {
            None => Some(1),
            Some(v) => v.parse().ok(),
        }
    }
}
