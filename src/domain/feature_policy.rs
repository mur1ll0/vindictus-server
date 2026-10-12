//! Immutable recovered FeatureMatrix subset. Version ordering uses finite Framework evidence.
use crate::error::Error;
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
struct VersionPair {
    current: String,
    value: String,
    enabled: bool,
}
pub struct FeaturePolicy {
    current: String,
    values: BTreeMap<String, String>,
    pairs: Vec<VersionPair>,
}
fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{0085}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}')
}
impl FeaturePolicy {
    /// Rows keep source overwrite order; keys are ordinal/case-sensitive. NULL becomes empty.
    pub fn new(rows: impl IntoIterator<Item = (String, Option<String>)>) -> Result<Self, Error> {
        let mut values = BTreeMap::new();
        let mut count = 0;
        for (key, value) in rows {
            count += 1;
            if count > 128
                || key.is_empty()
                || key.len() > 256
                || !key.is_ascii()
                || value.as_ref().is_some_and(|v| v.len() > 256)
            {
                return Err(Error::RewardPolicyExport);
            }
            values.insert(key, value.unwrap_or_default());
        }
        let current = values
            .get("CurrentVer")
            .filter(|v| !v.is_empty())
            .ok_or(Error::RewardPolicyExport)?
            .clone();
        let pairs = serde_json::from_str(include_str!("feature_version_pairs.json"))
            .map_err(|_| Error::FeatureVersionUnsupported)?;
        Ok(Self {
            current,
            values,
            pairs,
        })
    }
    /// Resolved ordinal map, for internal semantic replay binding; never log its values.
    pub(crate) fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }
    pub fn current_version(&self) -> &str {
        &self.current
    }
    pub fn get_string(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }
    pub fn get_integer(&self, key: &str) -> Result<i32, Error> {
        // NumberStyles.Integer permits ASCII whitespace and terminal NULs (Framework oracle).
        let text = self
            .get_string(key)
            .trim_end_matches('\0')
            .trim_matches(|c| matches!(c, ' ' | '\u{0009}'..='\u{000d}'));
        let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Ok(0);
        }
        text.parse().map_err(|_| Error::FeatureIntegerOverflow)
    }
    pub fn is_enabled(&self, expression: Option<&str>) -> Result<bool, Error> {
        let mut budget = 256;
        self.evaluate(expression.unwrap_or(""), 0, &mut budget)
    }
    fn evaluate(&self, expression: &str, depth: usize, budget: &mut usize) -> Result<bool, Error> {
        if depth > 32 || *budget == 0 || expression.len() > 512 {
            return Err(Error::FeatureExpression);
        }
        *budget -= 1;
        if expression.is_empty() {
            return Ok(true);
        }
        // The source splits BEFORE trimming and only recurses when >=2 nonempty parts exist.
        for (separator, any) in [("||", true), ("&&", false)] {
            if expression.contains(separator) {
                let parts: Vec<_> = expression
                    .split(separator)
                    .filter(|p| !p.is_empty())
                    .collect();
                if parts.len() >= 2 {
                    for part in parts {
                        let enabled = self.evaluate(part, depth + 1, budget)?;
                        if enabled == any {
                            return Ok(any);
                        }
                    }
                    return Ok(!any);
                }
            }
        }
        let expression = expression.trim_matches(whitespace);
        if expression.is_empty() {
            return Err(Error::FeatureExpression);
        }
        if let Some(rest) = expression.strip_prefix('!') {
            return Ok(!self.evaluate(rest, depth + 1, budget)?);
        }
        let value = self.get_string(expression);
        if value.is_empty() {
            return Ok(false);
        }
        self.pairs
            .iter()
            .find(|p| p.current == self.current && p.value == value)
            .map(|p| p.enabled)
            .ok_or(Error::FeatureVersionUnsupported)
    }
}
