//! The private naming profile: the recovered name rule and retention values.
//! Any name that passes the rule is free; it is unavailable only while another stored name is the
//! same under SQL_Latin1_General_CP1_CI_AS (operator decision, 26/10/2026). The proof's measured
//! equalities stay as the oracle of that comparison.
use super::{
    reward_policy::RewardPolicyPackage,
    user_export::{PrivateText, hex},
};
use crate::domain::character_name::NameRule;
use serde::Deserialize;
use std::io::Read;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};
#[derive(Debug, Error, PartialEq, Eq)]
#[error("character naming proof is invalid or outside its measured profile; values omitted")]
pub struct NameProofError;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    source_table: String,
    utf16_hex: PrivateText,
    equal: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    format_version: u32,
    candidate: PrivateText,
    content_database_sha256: String,
    original_config_sha256: String,
    language_tag: String,
    naming_rule: String,
    max_bytes: i32,
    repeating: i32,
    protect_period: i32,
    japanese: bool,
    /// The measured candidate's ForbiddenName result; free names do not use it.
    #[serde(rename = "forbidden")]
    _forbidden: bool,
    forbidden_filtered_rows: usize,
    forbidden_cultures: usize,
    sql_collation: String,
    references: Vec<Reference>,
}
pub struct NameProof {
    rule: NameRule,
    pub(crate) protect_period: i32,
    pub(crate) japanese: bool,
}
impl NameProof {
    pub fn read(reader: impl Read, content: &RewardPolicyPackage) -> Result<Self, NameProofError> {
        let mut input = Zeroizing::new(Vec::new());
        reader
            .take(1024 * 1024 + 1)
            .read_to_end(&mut input)
            .map_err(|_| NameProofError)?;
        if input.len() > 1024 * 1024 {
            return Err(NameProofError);
        }
        let raw: Raw = serde_json::from_slice(&input).map_err(|_| NameProofError)?;
        if raw.format_version != 1
            || raw.content_database_sha256 != content.manifest.database_sha256
            || raw.original_config_sha256 != content.manifest.config_sha256
            || raw.language_tag != "en-EU"
            || raw.sql_collation != "SQL_Latin1_General_CP1_CI_AS"
            || raw.references.len() > 1000
            || raw.max_bytes != 20
            || raw.repeating != -1
            || raw.protect_period != 30
            || raw.japanese
            || raw.forbidden_filtered_rows != 768
            || raw.forbidden_cultures != 859
        {
            return Err(NameProofError);
        }
        let rule = NameRule::recovered(&raw.naming_rule, raw.max_bytes, raw.repeating)
            .map_err(|_| NameProofError)?;
        let units = Zeroizing::new(raw.candidate.0.encode_utf16().collect::<Vec<_>>());
        if !rule
            .check_valid_name(Some(&units))
            .map_err(|_| NameProofError)?
        {
            return Err(NameProofError);
        }
        let candidate_bytes = Zeroizing::new(
            units
                .iter()
                .flat_map(|u| u.to_le_bytes())
                .collect::<Vec<_>>(),
        );
        let mut references: Vec<(Zeroizing<Vec<u8>>, bool)> = Vec::new();
        for row in raw.references {
            if !matches!(
                row.source_table.as_str(),
                "CharacterInfo" | "ReservedName" | "CharacterNameLog" | "MeasuredSynthetic"
            ) {
                return Err(NameProofError);
            }
            let bytes = Zeroizing::new(hex(row.utf16_hex, 100).map_err(|_| NameProofError)?);
            if !bytes.len().is_multiple_of(2)
                || references
                    .iter()
                    .any(|(b, e)| b.as_slice() == bytes.as_slice() && *e != row.equal)
                || (bytes.as_slice() == candidate_bytes.as_slice() && !row.equal)
            {
                return Err(NameProofError);
            }
            if same_name(&raw.candidate.0, &bytes) != row.equal {
                return Err(NameProofError);
            }
            references.push((bytes, row.equal));
        }
        Ok(Self {
            rule,
            protect_period: raw.protect_period,
            japanese: raw.japanese,
        })
    }
    /// The recovered name rule; every name that passes it may be checked against stored names.
    pub(crate) fn valid_request(&self, name: Option<&str>) -> Result<bool, NameProofError> {
        let units = name.map(|s| Zeroizing::new(s.encode_utf16().collect::<Vec<_>>()));
        self.rule
            .check_valid_name(units.as_deref().map(|u| u.as_slice()))
            .map_err(|_| NameProofError)
    }
    /// SQL `=` under SQL_Latin1_General_CP1_CI_AS for a stored UTF-16LE name.
    pub(crate) fn equals(&self, name: &str, stored: &[u8]) -> bool {
        same_name(name, stored)
    }
    /// Ordinal equality (List.Contains), as the own reservation list compares.
    pub(crate) fn ordinal(&self, name: &str, stored: &[u8]) -> bool {
        stored.len() == name.encode_utf16().count() * 2
            && name
                .encode_utf16()
                .zip(stored.as_chunks::<2>().0)
                .all(|(u, b)| u.to_le_bytes() == *b)
    }
}
/// Trailing spaces are padding and ASCII letters compare without case. The name rule admits only
/// ASCII letters and `_`, so every other unit must match exactly. Agrees with all measured SQL
/// equalities (8 synthetic, 49 original).
fn same_name(name: &str, stored: &[u8]) -> bool {
    if !stored.len().is_multiple_of(2) {
        return false;
    }
    let trim = |units: &mut Vec<u16>| {
        while units.last() == Some(&0x20) {
            units.pop();
        }
    };
    let mut left = name.encode_utf16().collect::<Vec<_>>();
    let mut right = stored
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes(*b))
        .collect::<Vec<_>>();
    trim(&mut left);
    trim(&mut right);
    let same = left.len() == right.len()
        && left.iter().zip(&right).all(|(a, b)| {
            a == b
                || (*a < 0x80
                    && *b < 0x80
                    && (*a as u8).eq_ignore_ascii_case(&(*b as u8)))
        });
    left.zeroize();
    right.zeroize();
    same
}
