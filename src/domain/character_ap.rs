//! Recovered AP manager transitions; state is submitted by the owning character context.
use super::{character_stats::Arithmetic, legacy_user_data::SqlDateTime2, skill_runtime::Cache};
use thiserror::Error;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ApError {
    #[error("AP UTC provider failed")]
    Clock,
    #[error("AP maximum policy failed")]
    Maximum,
    #[error("AP interval provider failed")]
    Interval,
    #[error("AP timestamp exceeds the source DateTime range")]
    DateRange,
}
pub trait Inputs {
    fn utc(&mut self) -> Result<SqlDateTime2, ApError>;
    fn maximum(&mut self) -> Result<i32, ApError>;
    /// Includes source Stat[INT] access and SkillEffect.CalculateAPTime.
    fn interval(&mut self) -> Result<i32, ApError>;
}
#[derive(Clone, Copy)]
pub struct Snapshot {
    pub ap: i32,
    pub last_update: SqlDateTime2,
    pub bonus_seconds: i32,
}
pub struct State {
    snapshot: Snapshot,
}
impl State {
    /// APManager constructor initializes BonusAPTime=-1; AP/date belong to CharacterInfo.
    pub fn fresh(ap: i32, last_update: SqlDateTime2) -> Self {
        Self::restore(Snapshot {
            ap,
            last_update,
            bonus_seconds: -1,
        })
    }
    pub fn restore(snapshot: Snapshot) -> Self {
        Self { snapshot }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.snapshot
    }
    pub fn update(&mut self, inputs: &mut impl Inputs) -> Result<(), ApError> {
        let s = &mut self.snapshot;
        if s.bonus_seconds > 0 {
            let elapsed = inputs.utc()?.ticks() - s.last_update.ticks();
            // Framework TimeSpan.TotalSeconds multiplies ticks by its Double scale.
            let periods = clr_i32((elapsed as f64 * 0.0000001) / f64::from(s.bonus_seconds));
            let maximum = inputs.maximum()?;
            if maximum < 0 {
                s.ap = s.ap.wrapping_add(periods);
            } else if s.ap < maximum {
                s.ap = maximum.min(s.ap.wrapping_add(periods));
            }
            // Multiplication is Int32 before TimeSpan construction. AP already changed
            // if the following DateTime addition fails; do not roll it back here.
            let seconds = s.bonus_seconds.wrapping_mul(periods);
            let next = s
                .last_update
                .ticks()
                .checked_add(i64::from(seconds) * 10_000_000)
                .ok_or(ApError::DateRange)?;
            s.last_update = SqlDateTime2::from_ticks(next).map_err(|_| ApError::DateRange)?;
        }
        Ok(())
    }
    pub fn recalculate(&mut self, inputs: &mut impl Inputs) -> Result<(), ApError> {
        if self.snapshot.bonus_seconds > 0 {
            self.update(inputs)?;
        } else if self.snapshot.bonus_seconds == 0 {
            self.snapshot.last_update = inputs.utc()?;
        }
        self.snapshot.bonus_seconds = inputs.interval()?;
        Ok(())
    }
    pub fn use_ap(&mut self, value: i32, inputs: &mut impl Inputs) -> Result<bool, ApError> {
        self.update(inputs)?;
        if value > self.snapshot.ap {
            return Ok(false);
        }
        self.snapshot.ap = self.snapshot.ap.wrapping_sub(value);
        Ok(true)
    }
    pub fn give(
        &mut self,
        value: i32,
        allow_exceed: bool,
        inputs: &mut impl Inputs,
    ) -> Result<i32, ApError> {
        self.update(inputs)?;
        if allow_exceed {
            self.snapshot.ap = self.snapshot.ap.wrapping_add(value);
            return Ok(value);
        }
        let given = if inputs.maximum()? >= 0 {
            0.max(value.min(inputs.maximum()?.wrapping_sub(self.snapshot.ap)))
        } else {
            value
        };
        // Source compares an already-overflowed Int32 sum with Int32.MaxValue.
        // Preserve its unchecked addition rather than introducing saturation.
        self.snapshot.ap = self.snapshot.ap.wrapping_add(given);
        Ok(given)
    }
}
fn clr_i32(value: f64) -> i32 {
    let n = value.trunc();
    if n.is_finite() && (f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&n) {
        n as i32
    } else {
        i32::MIN
    }
}
/// SkillEffect.Arg("skill_meditation", "meditation_interval_reduction", -1): -1 when
/// meditation is not learned (a present dictionary with a missing key reads 0).
pub fn meditation_reduction(cache: &Cache) -> i32 {
    let text = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    cache.argument(
        &text("skill_meditation"),
        &text("meditation_interval_reduction"),
        -1,
    )
}

/// FeatureMatrix.GetFloat: float.Parse of the value (invariant digits, sign, decimal
/// point and exponent); a FormatException, including a missing key (""), returns 0.
pub fn feature_float(value: &str) -> f32 {
    let text = value.trim_matches(|c| matches!(c, ' ' | '\u{0009}'..='\u{000d}'));
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return 0.0;
    }
    text.parse().unwrap_or(0.0)
}

/// SkillEffect.CalculateAPTime(INT): 0 without meditation; otherwise
/// 3600 - reduction - min(300, (int)(INT * StatRatio * 300 / 2000)), at least 5.
/// x64 rounds every Single operation; the x86 JIT keeps x87 extended intermediates.
pub fn ap_interval(reduction: i32, int_stat: i32, stat_ratio: f32, arithmetic: Arithmetic) -> i32 {
    if reduction == -1 {
        return 0;
    }
    let ratio = if stat_ratio == 0.0 { 1.0 } else { stat_ratio };
    let bonus = match arithmetic {
        Arithmetic::FrameworkX64 => clr_i32(f64::from(int_stat as f32 * ratio * 300.0 / 2000.0)),
        Arithmetic::FrameworkX86 => {
            clr_i32(f64::from(int_stat) * f64::from(ratio) * 300.0 / 2000.0)
        }
    };
    3600_i32
        .wrapping_sub(reduction)
        .wrapping_sub(bonus.min(300))
        .max(5)
}

/// APMessage/APInfo timing: (long)(LastAPUpdate + BonusAPTime s - UtcNow).TotalMilliseconds
/// and the interval; a non-positive interval reports -1 for both.
pub fn timing(last_update: SqlDateTime2, interval: i32, now: SqlDateTime2) -> (i64, i32) {
    if interval <= 0 {
        return (-1, -1);
    }
    let ticks = last_update
        .ticks()
        .wrapping_add(i64::from(interval) * 10_000_000)
        .wrapping_sub(now.ticks());
    // TimeSpan.TotalMilliseconds: ticks * 0.0001 as Double, truncated by the cast.
    ((ticks as f64 * 0.0001) as i64, interval)
}
