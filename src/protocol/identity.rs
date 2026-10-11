//! Laboratory identity expressions recovered from PassportCheck.OnEnter.
//!
//! This is a compatibility calculation, not proof of authentication or a password hash.
//! Callers must select an explicitly verified nonrandomized Framework runtime profile.
use thiserror::Error;

/// Legacy Framework runtime architecture; hashes differ across architectures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameworkArchitecture {
    /// Windows 32-bit Framework hash over packed UTF-16 pairs.
    X86,
    /// Windows 64-bit Framework hash over alternating UTF-16 units.
    X64,
}

/// Cultures verified to share ASCII casing in the synthetic Framework oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AsciiCulture {
    /// Explicit invariant culture, not a claim about the original process culture.
    Invariant,
    /// English (United States).
    EnUs,
    /// Chinese (Taiwan).
    ZhTw,
    /// Chinese (China).
    ZhCn,
}

/// Explicit lab profile; there is deliberately no inferred or default profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentityProfile {
    /// Framework process architecture with randomized string hashing disabled.
    pub architecture: FrameworkArchitecture,
    /// Culture whose ASCII casing has been verified.
    pub culture: AsciiCulture,
}

/// Defensive parsing policy and recovered FeatureMatrix flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabIdentityOptions {
    /// Explicit verified Framework profile.
    pub profile: IdentityProfile,
    /// Laboratory branch is available only when NGM_disable is enabled.
    pub ngm_disable: bool,
    /// Apply the additional zhCN test account restrictions.
    pub zh_cn: bool,
    /// Reject overlong input before allocating; this is a new defensive limit.
    pub max_passport_bytes: usize,
}

/// Recovered fields without inherited legacy permissions or credentials.
#[derive(PartialEq, Eq)]
pub struct LabIdentity {
    /// Original case preserved, truncated to 24 ASCII/UTF-16 units.
    pub nexon_id: String,
    /// Signed Framework hash of the entire lowercased passport, before truncation.
    pub nexon_sn: i32,
}

/// Lab identity rejection; messages intentionally do not contain the passport.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    /// Online passport verification is outside this compatibility module.
    #[error("NGM_disable is not enabled")]
    OnlineMode,
    /// Null passport is rejected rather than reproducing a legacy null dereference.
    #[error("null passport")]
    NullPassport,
    /// Additional defensive length policy.
    #[error("passport exceeds configured limit")]
    TooLarge,
    /// Unicode or control characters require a separate proven casing contract.
    #[error("passport is outside the verified printable ASCII contract")]
    UnsupportedCharacters,
    /// Legacy empty-passport failure in the generic laboratory branch.
    #[error("PassportFail")]
    PassportFail,
    /// Legacy restricted test-account failure in the zhCN branch.
    #[error("NoUserInfo")]
    NoUserInfo,
}

/// Calculate laboratory identity for printable ASCII only.
///
/// All supported cultures have identical casing within this restricted contract.
/// This does not verify a passport, create an account, or grant legacy permissions.
pub fn derive_lab_identity(
    passport: Option<&str>,
    options: LabIdentityOptions,
) -> Result<LabIdentity, IdentityError> {
    if !options.ngm_disable {
        return Err(IdentityError::OnlineMode);
    }
    let passport = passport.ok_or(IdentityError::NullPassport)?;
    if passport.len() > options.max_passport_bytes {
        return Err(IdentityError::TooLarge);
    }
    if !passport.bytes().all(|byte| (32..=126).contains(&byte)) {
        return Err(IdentityError::UnsupportedCharacters);
    }
    let lowercase = passport.to_ascii_lowercase();
    if options.zh_cn && !is_zh_cn_test_account(&lowercase) {
        return Err(IdentityError::NoUserInfo);
    }
    if passport.is_empty() {
        return Err(IdentityError::PassportFail);
    }
    let utf16: Vec<u16> = lowercase.encode_utf16().collect();
    Ok(LabIdentity {
        nexon_id: passport[..passport.len().min(24)].to_owned(),
        nexon_sn: framework_string_hash(&utf16, options.profile.architecture),
    })
}

fn is_zh_cn_test_account(lowercase: &str) -> bool {
    let (suffix, length, maximum) = if let Some(suffix) = lowercase.strip_prefix("cmhtest") {
        (suffix, 2, 60)
    } else if let Some(suffix) = lowercase.strip_prefix("whb") {
        (suffix, 3, 6)
    } else {
        return false;
    };
    if suffix.len() != length {
        return false;
    }
    // Framework Int32.Parse's verified ASCII subset allows surrounding spaces and signs.
    suffix
        .trim_matches(' ')
        .parse::<i32>()
        .is_ok_and(|number| (1..=maximum).contains(&number))
}

/// Reproduce nonrandomized release Framework GetHashCode over UTF-16 units.
///
/// Casing belongs to the caller. The x64 algorithm stops at a NUL unit; x86 uses
/// the supplied length and packs little-endian unit pairs, including a final zero.
/// Do not use this noncryptographic 32-bit hash for authentication or new identities.
pub fn framework_string_hash(units: &[u16], architecture: FrameworkArchitecture) -> i32 {
    match architecture {
        FrameworkArchitecture::X86 => {
            let mut hash1: i32 = (5381 << 16) + 5381;
            let mut hash2 = hash1;
            let mut index = 0;
            while units.len().saturating_sub(index) > 2 {
                hash1 = mix_x86(hash1) ^ packed_pair(units, index);
                hash2 = mix_x86(hash2) ^ packed_pair(units, index + 2);
                index += 4;
            }
            if index < units.len() {
                hash1 = mix_x86(hash1) ^ packed_pair(units, index);
            }
            hash1.wrapping_add(hash2.wrapping_mul(1_566_083_941))
        }
        FrameworkArchitecture::X64 => {
            let mut hash1: i32 = 5381;
            let mut hash2 = hash1;
            for pair in units.chunks(2) {
                if pair[0] == 0 {
                    break;
                }
                hash1 = hash1.wrapping_mul(33) ^ i32::from(pair[0]);
                if pair.len() == 1 || pair[1] == 0 {
                    break;
                }
                hash2 = hash2.wrapping_mul(33) ^ i32::from(pair[1]);
            }
            hash1.wrapping_add(hash2.wrapping_mul(1_566_083_941))
        }
    }
}

fn mix_x86(hash: i32) -> i32 {
    // Signed arithmetic right shift is essential; wrapping_add reproduces unchecked overflow.
    hash.wrapping_shl(5)
        .wrapping_add(hash)
        .wrapping_add(hash >> 27)
}

fn packed_pair(units: &[u16], index: usize) -> i32 {
    let low = u32::from(units.get(index).copied().unwrap_or(0));
    let high = u32::from(units.get(index + 1).copied().unwrap_or(0));
    (low | (high << 16)) as i32
}
