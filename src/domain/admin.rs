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

pub const LAUNCHER_TICKET_PREFIX: &str = "lt";
const LAUNCHER_TICKET_DIGITS: usize = 22;
const BASE36: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";

/// One-time game login ticket the launcher gets for login and password and hands to the client
/// (`-launcher_ticket`), which sends it as the UserLogin passport. `lt` plus 22 lowercase base36
/// digits (about 113 random bits): 24 characters, the size of a typed ID, and no case the client
/// could fold. Not Debug: it is a credential.
pub struct LauncherTicket(Zeroizing<String>);

impl LauncherTicket {
    pub fn generate() -> Self {
        let mut text = Zeroizing::new(String::with_capacity(24));
        text.push_str(LAUNCHER_TICKET_PREFIX);
        let mut pool = Zeroizing::new([0u8; 64]);
        while text.len() < LAUNCHER_TICKET_PREFIX.len() + LAUNCHER_TICKET_DIGITS {
            OsRng.fill_bytes(pool.as_mut());
            // 252 = 7 * 36: rejecting the top four values keeps every digit equally likely.
            for byte in pool.iter().filter(|b| **b < 252) {
                if text.len() == LAUNCHER_TICKET_PREFIX.len() + LAUNCHER_TICKET_DIGITS {
                    break;
                }
                text.push(char::from(BASE36[usize::from(byte % 36)]));
            }
        }
        Self(text)
    }
    /// Accepts exactly the generated form; anything else is an ordinary typed ID.
    pub fn parse(value: &str) -> Option<Self> {
        value
            .strip_prefix(LAUNCHER_TICKET_PREFIX)
            .filter(|digits| {
                digits.len() == LAUNCHER_TICKET_DIGITS
                    && digits
                        .bytes()
                        .all(|b| b.is_ascii_digit() || b.is_ascii_lowercase())
            })
            .map(|_| Self(Zeroizing::new(value.to_owned())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// The stored form: the database never holds the ticket itself.
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
    fn launcher_tickets_are_24_lowercase_characters_and_parse_back() {
        let a = LauncherTicket::generate();
        let b = LauncherTicket::generate();
        assert_eq!(a.as_str().len(), 24);
        assert!(a.as_str().starts_with(LAUNCHER_TICKET_PREFIX));
        assert_ne!(a.as_str(), b.as_str());
        let parsed = LauncherTicket::parse(a.as_str()).expect("generated form parses");
        assert_eq!(parsed.digest(), a.digest());
        assert!(LauncherTicket::parse("admin").is_none());
        assert!(LauncherTicket::parse(&a.as_str().to_uppercase()).is_none());
        assert!(LauncherTicket::parse(&a.as_str()[..23]).is_none());
        assert!(LauncherTicket::parse(&format!("{}0", a.as_str())).is_none());
    }

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
