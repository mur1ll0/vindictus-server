//! A finite, private comparison proof for one explicitly measured lab name.
//! Unknown requested/stored values are refused, never classified as available.
use super::{
    reward_policy::RewardPolicyPackage,
    user_export::{PrivateText, hex},
};
use crate::domain::character_name::NameRule;
use serde::Deserialize;
use std::io::Read;
use thiserror::Error;
use zeroize::Zeroizing;
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
    forbidden: bool,
    forbidden_filtered_rows: usize,
    forbidden_cultures: usize,
    sql_collation: String,
    references: Vec<Reference>,
}
pub struct NameProof {
    candidate: Zeroizing<String>,
    candidate_bytes: Zeroizing<Vec<u8>>,
    references: Vec<(Zeroizing<Vec<u8>>, bool)>,
    rule: NameRule,
    pub(crate) protect_period: i32,
    pub(crate) japanese: bool,
    pub(crate) forbidden: bool,
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
        let candidate = Zeroizing::new(raw.candidate.0.clone());
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
            references.push((bytes, row.equal));
        }
        Ok(Self {
            candidate,
            candidate_bytes,
            references,
            rule,
            protect_period: raw.protect_period,
            japanese: raw.japanese,
            forbidden: raw.forbidden,
        })
    }
    /// Invalid format can be decided for all inputs; valid inputs must be the measured candidate.
    pub(crate) fn valid_request(&self, name: Option<&str>) -> Result<bool, NameProofError> {
        let units = name.map(|s| Zeroizing::new(s.encode_utf16().collect::<Vec<_>>()));
        if !self
            .rule
            .check_valid_name(units.as_deref().map(|u| u.as_slice()))
            .map_err(|_| NameProofError)?
        {
            return Ok(false);
        }
        if name != Some(self.candidate.as_str()) {
            return Err(NameProofError);
        }
        Ok(true)
    }
    pub(crate) fn equals(&self, value: &[u8]) -> Result<bool, NameProofError> {
        if value == self.candidate_bytes.as_slice() {
            return Ok(true);
        }
        self.references
            .iter()
            .find(|(b, _)| b.as_slice() == value)
            .map(|(_, eq)| *eq)
            .ok_or(NameProofError)
    }
    pub(crate) fn ordinal_candidate(&self, value: &[u8]) -> bool {
        value == self.candidate_bytes.as_slice()
    }
}
