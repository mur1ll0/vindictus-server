//! nmcogame.dll of the laboratory (B5 MS2, decision D-S1; docs/messenger-protocol.md,
//! docs/messenger-design.md): the six exports engine.dll imports by ordinal. The local calls
//! (callback registration, Initialize, locale and code page) succeed here with a minimal reply
//! and keep the windows events go to; every other call goes through the `tunnel` to the server's
//! messenger service, whose events are posted to those windows. Without the server those calls
//! fail, which leaves the messenger off. Every call is appended to `nmcogame-lab.log` in the game
//! folder (code, envelope header and, except for the authentication codes, the first bytes).
//!
//! - `ffi`: the only module with `unsafe` (the exported functions, raw pointers, PostMessage
//!   and the buffers handed to the engine).
//! - the rest is safe code: the envelope, the replies, the tunnel and the log.
#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ffi;
mod tunnel;

use std::io::Write;

/// Envelope v0 (CStreamCryptor_v0): magic, length, plain body, trailer.
const V0_MAGIC: u32 = 0x001F_CA34;
const V0_TRAILER: u32 = 0x008A_119E;
/// Envelope v1 (CStreamCryptor_v1, what the engine sends).
const V1_MAGIC: u32 = 0x0A98_C32A;
const V1_TRAILER: u32 = 0x820A_9E12;
/// Event object header: type word 'eo', b6 1, version 1.
const EVENT_OBJECT: u32 = 0x656f_0101;
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

fn seal(object: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(object.len() + 12);
    out.extend_from_slice(&V0_MAGIC.to_le_bytes());
    out.extend_from_slice(&(object.len() as u32).to_le_bytes());
    out.extend_from_slice(object);
    out.extend_from_slice(&V0_TRAILER.to_le_bytes());
    out
}

/// An event envelope with one u32 field (MessengerReply, MsgConnectionClosed).
pub fn event_reply(value: u32) -> Vec<u8> {
    let mut object = Vec::with_capacity(12);
    object.extend_from_slice(&EVENT_OBJECT.to_le_bytes());
    object.extend_from_slice(&4u32.to_le_bytes());
    object.extend_from_slice(&value.to_le_bytes());
    seal(&object)
}

/// The body of a v0 or v1 envelope (the v1 checksum is not checked).
pub fn open(envelope: &[u8]) -> Option<Vec<u8>> {
    let word = |at: usize| {
        envelope
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (magic, len) = (word(0)?, word(4)? as usize);
    let body = envelope.get(8..8 + len)?;
    match (magic, word(8 + len)?) {
        (V0_MAGIC, V0_TRAILER) => Some(body.to_vec()),
        (V1_MAGIC, V1_TRAILER) => {
            let tmp: Vec<u8> = body.get(..body.len().checked_sub(2)?)?.iter().rev().copied().collect();
            if tmp.len() < 8 || tmp[1] != 1 {
                return None;
            }
            let key = tmp[3];
            Some(
                (8..tmp.len())
                    .map(|i| tmp[i] ^ key ^ if i == 8 { 0 } else { tmp[i - 1] })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// The u32 words of a call object's body (callArg first), up to 8.
pub fn call_words(envelope: &[u8]) -> Vec<u32> {
    let Some(body) = open(envelope) else {
        return Vec::new();
    };
    body.get(8..)
        .unwrap_or_default()
        .as_chunks::<4>()
        .0
        .iter()
        .take(8)
        .map(|w| u32::from_le_bytes(*w))
        .collect()
}

/// A call: local ones (with their callback windows kept) answer here, the others go to the
/// server. None makes NMCO_CallNMFunction return FALSE.
pub fn dispatch(code: u32, envelope: &[u8]) -> Option<Vec<u8>> {
    if LOCAL.contains(&code) {
        if matches!(code, 0x1002 | 0xc001 | 0xb001) {
            tunnel::register(code, &call_words(envelope));
        }
        if code == 0xc001 {
            // The server marks the member's session window registered (then 0xa002 follows).
            let _ = tunnel::call(code, envelope);
        }
        return answer(code);
    }
    match tunnel::call(code, envelope) {
        Some((true, reply)) => Some(reply),
        _ => None,
    }
}

/// Sets the function events are posted with.
pub fn set_post(post: tunnel::Post) {
    tunnel::set_post(post);
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
    fn envelopes_open_and_registration_words_read() {
        // RegisterCallback {callArg 0, hWnd 0x1234, 0x8001} in a v0 envelope.
        let mut object = 0x666f_0101u32.to_le_bytes().to_vec();
        object.extend_from_slice(&12u32.to_le_bytes());
        for w in [0u32, 0x1234, 0x8001] {
            object.extend_from_slice(&w.to_le_bytes());
        }
        assert_eq!(call_words(&seal(&object)), [0, 0x1234, 0x8001]);
        assert_eq!(open(&event_reply(0)).map(|b| b.len()), Some(12));
        assert!(open(&[1, 2, 3]).is_none());
    }

    #[test]
    fn credentials_are_not_logged() {
        let line = log_line(0x110f, 1, &[0x2a, 0xc3, 0x98, 0x0a, 9, 0, 0, 0, 0xff], false);
        assert!(line.contains("magic=0x0a98c32a len=9") && !line.contains("head="));
        assert!(log_line(0x4001, 1, &[1, 2], false).ends_with("head=0102"));
    }
}
