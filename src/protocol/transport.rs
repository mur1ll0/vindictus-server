//! Stateful recovered Devcat transport, independently seeded per direction.
//! The first packet replaces its instance id with a clear big-endian salt.
//! This implements transport bytes only; message negotiation and game login remain separate.
use super::isaac::LegacyIsaac;
use super::{Frame, ProtocolError, decode_frame};
use thiserror::Error;
use zeroize::Zeroize;

/// Choose the recovered cipher explicitly; the two seed initializations differ.
#[derive(Clone, Copy)]
pub enum CipherProfile {
    /// Devcat.Core.Net.Message.CryptoTransform, used by generic Core transport.
    DevcatCore,
    /// ServiceCore.CryptoTransformHeroes, selected by FrontendService.OnClientAccept.
    VindictusFrontend,
}
impl CipherProfile {
    fn initialize(self, salt: u32) -> LegacyIsaac {
        match self {
            Self::DevcatCore => LegacyIsaac::seeded(salt as i32),
            Self::VindictusFrontend => LegacyIsaac::heroes_seeded(salt),
        }
    }
}
/// Invalid transport state or packet.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransportError {
    /// Sender salts come from System.Random.Next (nonnegative signed int).
    #[error("salt exceeds legacy sender range")]
    InvalidSalt,
    /// A parser error permanently invalidates this stream.
    #[error("stream is closed after invalid input")]
    Closed,
    /// Invalid decoded framing.
    #[error(transparent)]
    Packet(#[from] ProtocolError),
}

/// Frame detached from the decoder's bounded pending buffer.
#[derive(PartialEq, Eq)]
pub struct OwnedFrame {
    /// The first encrypted frame is delivered with instance id zero by the legacy analyzer.
    pub instance_id: i64,
    /// Registered message category; this module does not assign categories.
    pub category_id: i32,
    /// Serialized message bytes.
    pub body: Vec<u8>,
}

impl std::fmt::Debug for OwnedFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedFrame")
            .field("category_id", &self.category_id)
            .field("body_len", &self.body.len())
            .finish_non_exhaustive()
    }
}
impl Drop for OwnedFrame {
    fn drop(&mut self) {
        self.body.zeroize();
    }
}

/// One outgoing direction. Choose a fresh salt for each connection externally.
/// ISAAC is retained for compatibility, not used as modern application security.
pub struct LegacySender {
    cipher: LegacyIsaac,
    salt: u32,
    first: bool,
    max_body: usize,
}
impl LegacySender {
    /// Initialize generic Core transport (not the Vindictus frontend) with an explicit salt.
    pub fn new(salt: u32, max_body: usize) -> Result<Self, TransportError> {
        Self::with_profile(CipherProfile::DevcatCore, salt, max_body)
    }
    /// Initialize a specific recovered transport profile. The game frontend uses VindictusFrontend.
    pub fn with_profile(
        profile: CipherProfile,
        salt: u32,
        max_body: usize,
    ) -> Result<Self, TransportError> {
        i32::try_from(salt).map_err(|_| TransportError::InvalidSalt)?;
        Ok(Self {
            cipher: profile.initialize(salt),
            salt,
            first: true,
            max_body,
        })
    }
    /// Encrypt a complete frame. Rejected frames do not consume stream state.
    pub fn encode(&mut self, frame: &Frame<'_>) -> Result<Vec<u8>, TransportError> {
        let mut bytes = frame.encode(self.max_body)?;
        self.cipher.xor_bytes(&mut bytes);
        if self.first {
            bytes[..8].copy_from_slice(&i64::from(self.salt).to_be_bytes());
            self.first = false;
        }
        Ok(bytes)
    }
}

/// One incoming direction, accepting fragmented or concatenated TCP data.
/// Buffering is limited to a single frame with the configured body limit plus its header.
/// On a parse error the session must close; state cannot be resynchronized safely.
pub struct LegacyReceiver {
    cipher: Option<LegacyIsaac>,
    profile: CipherProfile,
    pending: Vec<u8>,
    max_body: usize,
    closed: bool,
}
impl LegacyReceiver {
    /// Start generic Core transport (not the Vindictus frontend) with no salt yet.
    pub fn new(max_body: usize) -> Self {
        Self::with_profile(CipherProfile::DevcatCore, max_body)
    }
    /// Start a direction with the cipher actually selected by the reference service.
    pub fn with_profile(profile: CipherProfile, max_body: usize) -> Self {
        Self {
            profile,
            cipher: None,
            pending: Vec::new(),
            max_body,
            closed: false,
        }
    }
    /// Consume all supplied bytes and deliver complete decoded frames.
    /// Empty input has no effect. Incomplete frames remain buffered.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<OwnedFrame>, TransportError> {
        if self.closed {
            return Err(TransportError::Closed);
        }
        let result = self.consume(bytes);
        if result.is_err() {
            self.closed = true;
            self.pending.zeroize();
        }
        result
    }
    /// Number of bytes held for the incomplete frame.
    pub fn buffered_len(&self) -> usize {
        self.pending.len()
    }
    fn consume(&mut self, bytes: &[u8]) -> Result<Vec<OwnedFrame>, TransportError> {
        let mut frames = Vec::new();
        for &byte in bytes {
            if let Some(cipher) = &mut self.cipher {
                self.pending.push(byte ^ cipher.next_word() as u8);
            } else {
                self.pending.push(byte);
                if self.pending.len() == 8 {
                    let mut salt = [0; 8];
                    salt.copy_from_slice(&self.pending);
                    // C# truncates the long salt to 32 bits, consuming eight words
                    // even though these eight bytes were overwritten by Packet.Encrypt.
                    let mut cipher = self.profile.initialize(i64::from_be_bytes(salt) as u32);
                    cipher.xor_bytes(&mut self.pending);
                    self.pending.fill(0);
                    self.cipher = Some(cipher);
                }
                continue;
            }
            if let Some((frame, _)) = decode_frame(&self.pending, self.max_body)? {
                frames.push(OwnedFrame {
                    instance_id: frame.instance_id,
                    category_id: frame.category_id,
                    body: frame.body.to_vec(),
                });
                self.pending.zeroize();
            }
        }
        Ok(frames)
    }
}

impl Drop for LegacyReceiver {
    fn drop(&mut self) {
        self.pending.zeroize();
    }
}
