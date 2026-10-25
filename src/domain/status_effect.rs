//! StatusEffectGroupInfo rules of the character StatusEffectManager: the persistence kind,
//! StatAdder, the overwrite test and the client element of a stored effect.
use std::collections::BTreeMap;

/// StatusEffectGroupInfo.StatusEffectType (from ServerSide and RealTimePeriod).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Client,
    DeleteOnLogOut,
    UseRemainTime,
    UseExpireTime,
}

/// One StatusEffectGroupInfo row (Type, Level) of the exported subset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info {
    pub type_name: String,
    pub level: i32,
    pub abs_level: i32,
    pub id_tag: Option<String>,
    pub duration: Option<i32>,
    pub combat_count: i32,
    pub kind: Kind,
    /// StatAdder: stat name → amount.
    pub stat_adder: Vec<(String, i32)>,
}

impl Info {
    /// StatusEffectExtension.CanOverWrite: the same Type needs a level at least as high,
    /// the same IDTag an AbsLevel at least as high.
    pub fn can_overwrite(&self, target: &Info) -> bool {
        if self.type_name == target.type_name {
            return self.level >= target.level;
        }
        if self.id_tag.is_some() && self.id_tag == target.id_tag {
            return self.abs_level >= target.abs_level;
        }
        false
    }
}

/// CharacterContents.GetStatusEffectGroupInfo over the exported rows.
#[derive(Clone, Debug, Default)]
pub struct Contents(BTreeMap<(String, i32), Info>);

impl Contents {
    pub fn new(rows: impl IntoIterator<Item = Info>) -> Option<Self> {
        let mut map = BTreeMap::new();
        for info in rows {
            // ToDoubleDictionary(Type, Level) throws on a duplicate.
            if map
                .insert((info.type_name.clone(), info.level), info)
                .is_some()
            {
                return None;
            }
        }
        Some(Self(map))
    }
    pub fn get(&self, type_name: &str, level: i32) -> Option<&Info> {
        self.0.get(&(type_name.to_owned(), level))
    }
}

/// A stored effect (StatusEffect row) with its info.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    pub row_id: i64,
    pub info: Info,
    /// ExpireTime in .NET ticks; None without expiry.
    pub expire_ticks: Option<i64>,
    pub remain_micro: Option<i16>,
}

const TICKS_PER_SECOND: i64 = 10_000_000;

impl Active {
    /// StatusEffectData.ToStatusEffectElement: (Type, Level, RemainTime, CombatCount).
    pub fn element(&self, now_ticks: i64) -> (String, i32, i32, i32) {
        let remain = self.expire_ticks.map_or(-1, |e| {
            i32::try_from((e - now_ticks) / TICKS_PER_SECOND).unwrap_or(i32::MAX)
        });
        let combat = if self.info.combat_count >= 0 {
            self.remain_micro.map_or(0, i32::from)
        } else {
            -1
        };
        (self.info.type_name.clone(), self.info.level, remain, combat)
    }
    pub fn is_expired(&self, now_ticks: i64) -> bool {
        self.expire_ticks.is_some_and(|e| e < now_ticks)
    }
}

/// StatusEffectStat: the StatAdder of every active effect.
pub fn stat_adder(effects: &[Active]) -> Vec<(String, i32)> {
    let mut sum: BTreeMap<String, i32> = BTreeMap::new();
    for e in effects {
        for (k, v) in &e.info.stat_adder {
            let s = sum.entry(k.clone()).or_default();
            *s = s.saturating_add(*v);
        }
    }
    sum.into_iter().collect()
}

/// Seconds as .NET ticks.
pub fn seconds_to_ticks(seconds: i32) -> i64 {
    i64::from(seconds) * TICKS_PER_SECOND
}
