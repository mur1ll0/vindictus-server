//! Recovered CheckValidName for the observed EN-EU pattern, not full name availability.
use thiserror::Error;

pub const EN_EU_NAMING_RULE: &str = "^[a-zA-Z]{2,}_?[a-zA-Z]{2,}$";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NameRuleError {
    #[error("character naming pattern has no recovered implementation")]
    UnsupportedRule,
    #[error("the recovered Regex.IsMatch rejects a null name")]
    NullName,
    #[error("character name exceeds the Framework string length domain")]
    Length,
}

/// Values must come from the effective recovered feature policy. No inferred defaults.
#[derive(Clone, Copy, Debug)]
pub struct NameRule {
    max_bytes: i32,
    repeating: i32,
}
impl NameRule {
    pub fn recovered(pattern: &str, max_bytes: i32, repeating: i32) -> Result<Self, NameRuleError> {
        if pattern != EN_EU_NAMING_RULE {
            return Err(NameRuleError::UnsupportedRule);
        }
        Ok(Self {
            max_bytes,
            repeating,
        })
    }
    /// UTF-16 units preserve .NET Length and char iteration, including unpaired surrogates.
    pub fn check_valid_name(self, name: Option<&[u16]>) -> Result<bool, NameRuleError> {
        let name = name.ok_or(NameRuleError::NullName)?;
        let length = i32::try_from(name.len()).map_err(|_| NameRuleError::Length)?;
        // Framework's $ also matches before one terminal LF. Do not trim other whitespace.
        let matched = name.strip_suffix(&[10]).unwrap_or(name);
        if !matches_naming_rule(matched) {
            return Ok(false);
        }
        // With this recovered naming pattern, all matching units are ASCII letters/_/LF.
        // The original auxiliary ^[a-zA-Z0-9_]*$ also accepts the same terminal LF.
        if length > self.max_bytes {
            return Ok(false);
        }
        if self.repeating > 0 {
            let mut previous = None;
            let mut run = 0;
            for unit in name {
                run = if previous == Some(unit) { run + 1 } else { 1 };
                if run > self.repeating {
                    return Ok(false);
                }
                previous = Some(unit);
            }
        }
        Ok(true)
    }
}
fn ascii_letter(unit: &u16) -> bool {
    matches!(unit, 65..=90 | 97..=122)
}
fn matches_naming_rule(name: &[u16]) -> bool {
    if let Some(index) = name.iter().position(|unit| *unit == 95) {
        index >= 2
            && name.len() - index > 2
            && name[..index].iter().all(ascii_letter)
            && name[index + 1..].iter().all(ascii_letter)
    } else {
        name.len() >= 4 && name.iter().all(ascii_letter)
    }
}
