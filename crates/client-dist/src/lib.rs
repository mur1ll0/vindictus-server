//! Client distribution of the Vindictus launcher (docs/future/game-launcher.md 4.2-4.7 and
//! docs/future/client-distribution-storage.md): one zstd object per file, addressed by the SHA-256 of the
//! file as the player's disk holds it, a manifest per version and a version pointer, both signed with
//! Ed25519. Used by the server (`vindictus-server client-dist`) and, later, by the launcher (L3).
//!
//! No database and no network here: types, rules and codecs only.

pub mod codec;
pub mod layout;
pub mod manifest;
pub mod paths;
pub mod rules;
pub mod sign;

/// Errors of the distribution formats.
#[derive(Debug, thiserror::Error)]
pub enum DistError {
    /// A path outside the rules of [`paths::check`].
    #[error("invalid game path: {0}")]
    Path(String),
    /// Bad JSON or a value outside the format.
    #[error("invalid {0}")]
    Format(&'static str),
    /// A signature that does not verify with the pinned public key.
    #[error("signature of {0} does not verify")]
    Signature(&'static str),
    /// A key file of the wrong size.
    #[error("invalid key file")]
    Key,
    /// The decompressed object does not match its SHA-256 or size.
    #[error("object {0} does not match its SHA-256")]
    Digest(String),
    /// I/O of the codec.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// SHA-256 of bytes, lowercase hex.
pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(data))
}
