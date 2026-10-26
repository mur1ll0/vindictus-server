//! Administration panel invariants (B4): roles, permissions and audit text bounds.
//! Panel policy, not a recovered game rule.
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

/// Panel role of a game account; an account without a role has no panel access.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelRole {
    Admin,
    Gm,
    Leitura,
}

/// What an action needs: reading, changing game state, or administering the panel itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    Read,
    Manage,
    Administer,
}

impl PanelRole {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "gm" => Some(Self::Gm),
            "leitura" => Some(Self::Leitura),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Gm => "gm",
            Self::Leitura => "leitura",
        }
    }
    pub fn allows(self, permission: Permission) -> bool {
        match permission {
            Permission::Read => true,
            Permission::Manage => self != Self::Leitura,
            Permission::Administer => self == Self::Admin,
        }
    }
}

pub const MAX_REASON_CHARS: usize = 500;

/// Mandatory reason of an audited change: trimmed, 1–500 characters, no control characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditReason(String);

impl AuditReason {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        let count = value.chars().count();
        ((1..=MAX_REASON_CHARS).contains(&count) && !value.chars().any(char::is_control))
            .then(|| Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub const TOKEN_BYTES: usize = 32;
const CSRF_DOMAIN: &[u8] = b"vindictus-admin-csrf\0";

/// Random session token of the panel cookie. Not Debug: it is a credential.
pub struct SessionToken(Zeroizing<String>);

impl SessionToken {
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; TOKEN_BYTES]);
        OsRng.fill_bytes(bytes.as_mut());
        Self(Zeroizing::new(hex(bytes.as_ref())))
    }
    /// Accepts exactly the generated form (64 lowercase hex digits).
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == TOKEN_BYTES * 2
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| Self(Zeroizing::new(value.to_owned())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// The stored form: the database never holds the token itself.
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_bytes()).into()
    }
    /// Synchronizer CSRF token derived from the cookie token, so it needs no storage and a
    /// page that cannot read the HttpOnly cookie cannot compute it.
    pub fn csrf(&self) -> String {
        let mut hash = Sha256::new();
        hash.update(CSRF_DOMAIN);
        hash.update(self.0.as_bytes());
        hex(&hash.finalize())
    }
}

/// Symbols of a play code: no 0/O or 1/I, so it can be typed from the screen.
const CODE_ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 12;

/// One-time code typed in the client's ID field in place of a passport (60 random bits).
/// Not Debug: it is a credential until redeemed.
pub struct PlayCode(Zeroizing<String>);

impl PlayCode {
    pub fn generate() -> Self {
        let mut bytes = Zeroizing::new([0u8; CODE_LEN]);
        OsRng.fill_bytes(bytes.as_mut());
        let text = bytes
            .iter()
            .map(|b| char::from(CODE_ALPHABET[usize::from(b & 31)]))
            .collect();
        Self(Zeroizing::new(text))
    }
    /// Exactly 12 symbols of the alphabet, in any case (the client may change it).
    pub fn parse(value: &str) -> Option<Self> {
        let upper = value.to_ascii_uppercase();
        (upper.len() == CODE_LEN && upper.bytes().all(|b| CODE_ALPHABET.contains(&b)))
            .then(|| Self(Zeroizing::new(upper)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn digest(&self) -> [u8; 32] {
        Sha256::digest(self.0.as_bytes()).into()
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    text
}

/// Constant-time comparison for the CSRF header.
pub fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_grant_increasing_permissions() {
        assert!(PanelRole::Leitura.allows(Permission::Read));
        assert!(!PanelRole::Leitura.allows(Permission::Manage));
        assert!(PanelRole::Gm.allows(Permission::Manage));
        assert!(!PanelRole::Gm.allows(Permission::Administer));
        assert!(PanelRole::Admin.allows(Permission::Administer));
        assert_eq!(PanelRole::parse("gm"), Some(PanelRole::Gm));
        assert_eq!(PanelRole::parse("GM"), None);
    }

    #[test]
    fn reasons_are_bounded_and_trimmed() {
        assert_eq!(
            AuditReason::parse("  teste  ").map(|r| r.0),
            Some("teste".into())
        );
        assert!(AuditReason::parse("   ").is_none());
        assert!(AuditReason::parse("a\nb").is_none());
        assert!(AuditReason::parse(&"é".repeat(500)).is_some());
        assert!(AuditReason::parse(&"é".repeat(501)).is_none());
    }

    #[test]
    fn play_codes_are_typable_and_case_insensitive() {
        let code = PlayCode::generate();
        assert_eq!(code.as_str().len(), CODE_LEN);
        let lower = PlayCode::parse(&code.as_str().to_ascii_lowercase()).expect("any case");
        assert_eq!(lower.digest(), code.digest());
        assert!(PlayCode::parse("lab169_a").is_none());
        assert!(PlayCode::parse("ABCDEFGHJKL0").is_none());
        assert!(PlayCode::parse("ABCDEFGHJKLMN").is_none());
    }

    #[test]
    fn tokens_round_trip_and_derive_a_distinct_csrf() {
        let token = SessionToken::generate();
        let parsed = SessionToken::parse(token.as_str()).expect("generated form");
        assert_eq!(parsed.digest(), token.digest());
        assert_eq!(token.csrf().len(), 64);
        assert_ne!(token.csrf(), token.as_str());
        assert!(SessionToken::parse(&token.as_str().to_uppercase()).is_none());
        assert!(SessionToken::parse("abc").is_none());
        assert!(same_secret(&token.csrf(), &parsed.csrf()));
        assert!(!same_secret(
            &token.csrf(),
            &SessionToken::generate().csrf()
        ));
    }
}
