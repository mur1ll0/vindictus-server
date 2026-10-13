//! Bounded operator configuration for the future loopback laboratory frontend.
//! Reads protected configuration, not credentials/passports from original databases.
use crate::{
    domain::{game_user::LegacyNameKey, lab_account::LabAccountScope},
    infrastructure::{
        reward_policy::RewardPolicyPackage,
        user_export::{PrivateText, nullable},
    },
    protocol::identity::{
        AsciiCulture, FrameworkArchitecture, IdentityProfile, LabIdentityOptions,
        derive_lab_identity,
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{io::Read, net::SocketAddr};
use thiserror::Error;
use zeroize::Zeroizing;
pub const MAX_CONFIG_BYTES: usize = 16 * 1024;
pub const MAX_ALLOWED_PASSPORTS: usize = 8;
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeConfigError {
    #[error("invalid or oversized laboratory configuration; private values omitted")]
    Input,
    #[error("laboratory requires explicit loopback/runtime/content profile")]
    Profile,
    #[error("laboratory allowlist is invalid, ambiguous or empty; private values omitted")]
    Allowlist,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    format_version: u32,
    bind: String,
    framework_architecture: String,
    ascii_culture: String,
    content_database_sha256: String,
    original_config_sha256: String,
    region: String,
    #[serde(deserialize_with = "nullable")]
    override_region: Option<String>,
    game_code: i32,
    server_code: i32,
    current_version: String,
    service_id: i32,
    channel_code: u8,
    civil_offset_seconds: i32,
    ping_pong: bool,
    allowed_passports: Vec<PrivateText>,
}
/// Not Debug/Serialize: startup diagnostics cannot disclose the allowlist.
pub struct LabRuntimeConfig {
    bind: SocketAddr,
    identity: LabIdentityOptions,
    scope: LabAccountScope,
    civil_offset_seconds: i32,
    ping_pong: bool,
    passports: Vec<Zeroizing<String>>,
}
fn hash_bytes(value: &str) -> Result<[u8; 32], RuntimeConfigError> {
    if value.len() != 64 || !value.bytes().all(|v| v.is_ascii_hexdigit()) {
        return Err(RuntimeConfigError::Profile);
    }
    let mut bytes = [0; 32];
    for (i, part) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        let digit = |v: u8| {
            if v.is_ascii_digit() {
                v - b'0'
            } else {
                v.to_ascii_lowercase() - b'a' + 10
            }
        };
        bytes[i] = digit(part[0]) * 16 + digit(part[1]);
    }
    Ok(bytes)
}
fn hash_text(hash: &mut Sha256, text: &str) {
    hash.update((text.len() as u64).to_be_bytes());
    hash.update(text.as_bytes());
}
impl LabRuntimeConfig {
    /// Parse before binding or opening a database. The wrapper must protect the input ACL.
    pub fn read(
        reader: impl Read,
        content: &RewardPolicyPackage,
    ) -> Result<Self, RuntimeConfigError> {
        let mut input = Zeroizing::new(Vec::new());
        reader
            .take((MAX_CONFIG_BYTES + 1) as u64)
            .read_to_end(&mut input)
            .map_err(|_| RuntimeConfigError::Input)?;
        if input.len() > MAX_CONFIG_BYTES {
            return Err(RuntimeConfigError::Input);
        }
        let raw: RawConfig =
            serde_json::from_slice(&input).map_err(|_| RuntimeConfigError::Input)?;
        let bind: SocketAddr = raw.bind.parse().map_err(|_| RuntimeConfigError::Profile)?;
        let architecture = match raw.framework_architecture.as_str() {
            "x86" => FrameworkArchitecture::X86,
            "x64" => FrameworkArchitecture::X64,
            _ => return Err(RuntimeConfigError::Profile),
        };
        let culture = match raw.ascii_culture.as_str() {
            "invariant" => AsciiCulture::Invariant,
            "en-US" => AsciiCulture::EnUs,
            "zh-TW" => AsciiCulture::ZhTw,
            "zh-CN" => AsciiCulture::ZhCn,
            _ => return Err(RuntimeConfigError::Profile),
        };
        let db_hash = hash_bytes(&raw.content_database_sha256)?;
        let config_hash = hash_bytes(&raw.original_config_sha256)?;
        let m = &content.manifest;
        if raw.format_version != 1
            || !bind.ip().is_loopback()
            || bind.port() == 0
            || !(-50_400..=50_400).contains(&raw.civil_offset_seconds)
            || db_hash != hash_bytes(&m.database_sha256)?
            || config_hash != hash_bytes(&m.config_sha256)?
            || raw.region != m.region
            || raw.override_region != m.override_region
            || raw.game_code != m.game_code
            || raw.server_code != m.server_code
            || raw.current_version != content.policy.current_version()
            || !content
                .policy
                .is_enabled(Some("NGM_disable"))
                .map_err(|_| RuntimeConfigError::Profile)?
        {
            return Err(RuntimeConfigError::Profile);
        }
        // Each required gate must be declared in the recovered subset. Missing differs from disabled.
        for key in [
            "NGM_disable",
            "zhCN",
            "PassportImmigrator",
            "SecondPassword",
            "EssentialSecondPassword",
            "SecondPasswordClear",
            "SecondPassword_UsingSHA",
            "SecondPassword_IP_Limit",
            "MaxCharacterSlot",
            "UserCareSystem",
            "DisableInGameCheat",
            "DisableCharacterSlotCheat",
            "HomeCafeBonus",
            "CafeBonus",
            "CashShop",
            "FraudDetector",
            "NexonGameSecurity",
            "XignCode",
            "OldNexonIDProtect",
            "DisableLogging",
            "AsynchronousLogging",
            "HackshieldOff",
            "NotifyBanReason",
            "IPLockTime",
        ] {
            if !content.policy.entries().any(|(k, _)| k == key) {
                return Err(RuntimeConfigError::Profile);
            }
        }
        let identity = LabIdentityOptions {
            profile: IdentityProfile {
                architecture,
                culture,
            },
            ngm_disable: true,
            zh_cn: content
                .policy
                .is_enabled(Some("zhCN"))
                .map_err(|_| RuntimeConfigError::Profile)?,
            max_passport_bytes: 1024,
        };
        if raw.allowed_passports.is_empty() || raw.allowed_passports.len() > MAX_ALLOWED_PASSPORTS {
            return Err(RuntimeConfigError::Allowlist);
        }
        let mut passports = Vec::with_capacity(raw.allowed_passports.len());
        let mut ids = Vec::with_capacity(raw.allowed_passports.len());
        for mut value in raw.allowed_passports {
            let derived = derive_lab_identity(Some(&value.0), identity)
                .map_err(|_| RuntimeConfigError::Allowlist)?;
            let key = LegacyNameKey::parse(&derived.nexon_id)
                .map_err(|_| RuntimeConfigError::Allowlist)?;
            if key.as_str().is_empty()
                || ids.iter().any(|(id, name): &(i32, String)| {
                    *id == derived.nexon_sn || name == key.as_str()
                })
            {
                return Err(RuntimeConfigError::Allowlist);
            }
            ids.push((derived.nexon_sn, key.as_str().to_owned()));
            passports.push(Zeroizing::new(std::mem::take(&mut value.0)));
        }
        // Length-prefix strings and separate NULL from empty. No export-subset hash or allowlist
        // enters identity provenance; extending the list does not orphan its existing lab users.
        let mut hash = Sha256::new();
        hash.update(b"vindictus-local-lab-profile-v1\0");
        hash.update(db_hash);
        hash.update(config_hash);
        for text in [
            &raw.framework_architecture,
            &raw.ascii_culture,
            &raw.region,
            &raw.current_version,
        ] {
            hash_text(&mut hash, text);
        }
        match raw.override_region.as_deref() {
            None => hash.update([0]),
            Some(v) => {
                hash.update([1]);
                hash_text(&mut hash, v);
            }
        }
        for value in [
            raw.game_code,
            raw.server_code,
            raw.service_id,
            raw.civil_offset_seconds,
        ] {
            hash.update(value.to_be_bytes());
        }
        hash.update([raw.channel_code]);
        let scope = LabAccountScope::new(hash.finalize().into(), raw.service_id, raw.channel_code);
        Ok(Self {
            bind,
            identity,
            scope,
            civil_offset_seconds: raw.civil_offset_seconds,
            ping_pong: raw.ping_pong,
            passports,
        })
    }
    /// Exact source ASCII casing is verified; truncation/identity alone is not allowlist membership.
    pub fn permits(&self, passport: Option<&str>) -> bool {
        passport.is_some_and(|v| {
            v.len() <= self.identity.max_passport_bytes
                && self
                    .passports
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(v))
        })
    }
    pub fn allowed_count(&self) -> usize {
        self.passports.len()
    }
    pub fn bind(&self) -> SocketAddr {
        self.bind
    }
    pub fn identity_options(&self) -> LabIdentityOptions {
        self.identity
    }
    pub fn scope(&self) -> &LabAccountScope {
        &self.scope
    }
    pub fn civil_offset_seconds(&self) -> i32 {
        self.civil_offset_seconds
    }
    pub fn ping_pong(&self) -> bool {
        self.ping_pong
    }
}
