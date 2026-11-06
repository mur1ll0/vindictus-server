//! nmcogame.dll of the laboratory (B5 MS2a, decision D-S1; docs/messenger-protocol.md): the six
//! exports engine.dll imports by ordinal, without the Nexon Messenger behind them. Every call is
//! appended to `nmcogame-lab.log` in the game folder (code, envelope header and, except for the
//! authentication codes, the first bytes of the body) so an operator session gives the real
//! sequence. The local calls (callback registration, Initialize, locale and code page) succeed
//! with a minimal reply; everything else fails, which leaves the messenger off as without a
//! server.
//!
//! - `ffi`: the only module with `unsafe` (the exported functions and their raw pointers).
//! - the rest is safe code: the envelope, the reply and the log.
#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ffi;

use std::io::Write;

/// Envelope v0 (CStreamCryptor_v0): magic, length, plain body, trailer.
const V0_MAGIC: u32 = 0x001F_CA34;
const V0_TRAILER: u32 = 0x008A_119E;
/// Function object header of a version 1 reply: type word 'fo', b6 1, version 1.
const FUNCTION_REPLY: u32 = 0x666f_0101;

/// Calls answered locally with bSuccess = 1 and no result fields.
const LOCAL: [u32; 7] = [
    0x1002, // RegisterCallback
    0x1003, // ResetCallback
    0x1111, // Initialize
    0x2104, // SetLocale
    0x2109, // SetCodePage
    0xc001, // GSRegisterCallback
    0xb001, // chat room RegisterCallback
];
/// Calls whose body carries credentials: only their length is logged.
const SECRET: [u32; 4] = [0x110d, 0x110f, 0x1101, 0x1102];

/// The minimal v0 reply of a local call: `bSuccess = 1`.
pub fn success_reply() -> Vec<u8> {
    let mut object = Vec::with_capacity(12);
    object.extend_from_slice(&FUNCTION_REPLY.to_le_bytes());
    object.extend_from_slice(&4u32.to_le_bytes());
    object.extend_from_slice(&1u32.to_le_bytes());
    let mut out = Vec::with_capacity(object.len() + 12);
    out.extend_from_slice(&V0_MAGIC.to_le_bytes());
    out.extend_from_slice(&(object.len() as u32).to_le_bytes());
    out.extend_from_slice(&object);
    out.extend_from_slice(&V0_TRAILER.to_le_bytes());
    out
}

/// What a call does: the reply to copy into the engine's buffer, or a failure.
pub fn answer(code: u32) -> Option<Vec<u8>> {
    LOCAL.contains(&code).then(success_reply)
}

/// One log line of a call: code, crypt version, the envelope's magic and length, and (unless
/// the code carries credentials) the first 64 bytes of the envelope in hex.
pub fn log_line(code: u32, crypt: u8, envelope: &[u8], answered: bool) -> String {
    let word = |at: usize| {
        envelope
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let mut line = format!(
        "call code=0x{code:04x} crypt={crypt} magic={} len={} answered={answered}",
        word(0).map_or("-".to_owned(), |m| format!("0x{m:08x}")),
        word(4).map_or("-".to_owned(), |l| l.to_string()),
    );
    if !SECRET.contains(&code) {
        line.push_str(" head=");
        for b in envelope.iter().take(64) {
            line.push_str(&format!("{b:02x}"));
        }
    }
    line
}

/// Appends a line to `nmcogame-lab.log` in the current folder (the game's); failures are
/// ignored, the game must not depend on the log.
pub fn log(line: &str) {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("nmcogame-lab.log")
    {
        let _ = writeln!(f, "{seconds} {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_calls_get_a_v0_success_and_others_fail() {
        let reply = answer(0x1002).unwrap();
        assert_eq!(
            reply,
            [
                0x34, 0xca, 0x1f, 0x00, 12, 0, 0, 0, 0x01, 0x01, 0x6f, 0x66, 4, 0, 0, 0, 1, 0,
                0, 0, 0x9e, 0x11, 0x8a, 0x00
            ]
        );
        assert!(answer(0x4001).is_none());
        assert!(answer(0x110d).is_none());
    }

    #[test]
    fn credentials_are_not_logged() {
        let line = log_line(0x110f, 1, &[0x2a, 0xc3, 0x98, 0x0a, 9, 0, 0, 0, 0xff], false);
        assert!(line.contains("magic=0x0a98c32a len=9") && !line.contains("head="));
        assert!(log_line(0x4001, 1, &[1, 2], false).ends_with("head=0102"));
    }
}
