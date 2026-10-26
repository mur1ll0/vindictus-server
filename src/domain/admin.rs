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
