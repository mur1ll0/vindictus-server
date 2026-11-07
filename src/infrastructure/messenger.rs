//! The laboratory messenger service (B5 MS3b, decision D-S1; docs/messenger-design.md): the
//! NMCO calls the lab `nmcogame.dll` tunnels to the server. This first part keeps the passports
//! of accepted game logins, so a messenger LoginAuth (0x110f, the same passport the engine got
//! on its command line) authenticates the same account; a launcher ticket is spent by the game
//! login, so the messenger can only find it here. Only the SHA-256 of a passport is kept, for
//! ten minutes, and nothing is logged.
use std::{
    collections::HashMap,
    sync::{Mutex, PoisonError},
    time::{Duration, Instant},
};
use sha2::{Digest, Sha256};

/// How long an accepted login's passport authenticates the messenger.
const PASSPORT_LIFETIME: Duration = Duration::from_secs(600);
/// The registry's bound (a lab server; older entries are dropped first).
const MAX_PASSPORTS: usize = 4096;

fn digest(passport: &str) -> [u8; 32] {
    Sha256::digest(passport.as_bytes()).into()
}

/// The account a passport belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub nexon_sn: i32,
    pub nexon_id: String,
}

struct Entry {
    account: Account,
    at: Instant,
}

/// Passport → account of the recent game logins.
#[derive(Default)]
pub struct Passports {
    entries: Mutex<HashMap<[u8; 32], Entry>>,
}
impl Passports {
    /// After an accepted game login.
    pub fn remember(&self, passport: &str, account: Account, now: Instant) {
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries.retain(|_, e| now.duration_since(e.at) < PASSPORT_LIFETIME);
        if entries.len() >= MAX_PASSPORTS
            && let Some(oldest) = entries.iter().min_by_key(|(_, e)| e.at).map(|(k, _)| *k)
        {
            entries.remove(&oldest);
        }
        entries.insert(digest(passport), Entry { account, at: now });
    }
    /// The account of a messenger LoginAuth's passport, while it is fresh.
    pub fn account(&self, passport: &str, now: Instant) -> Option<Account> {
        let entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        entries
            .get(&digest(passport))
            .filter(|e| now.duration_since(e.at) < PASSPORT_LIFETIME)
            .map(|e| e.account.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passports_authenticate_for_ten_minutes() {
        let p = Passports::default();
        let now = Instant::now();
        let account = Account {
            nexon_sn: 31,
            nexon_id: "owner".to_owned(),
        };
        p.remember("ticket-1", account.clone(), now);
        assert_eq!(p.account("ticket-1", now + Duration::from_secs(599)), Some(account));
        assert_eq!(p.account("ticket-1", now + Duration::from_secs(600)), None);
        assert_eq!(p.account("other", now), None);
    }
}

/// A frame of the laboratory tunnel (docs/messenger-design.md): `u32 LE length`, `u8 kind`,
/// then the kind's payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// 1, DLL → server: sequence, function code, the engine's call envelope.
    Call {
        seq: u32,
        code: u32,
        envelope: Vec<u8>,
    },
    /// 2, server → DLL: sequence, success, the reply envelope.
    Reply {
        seq: u32,
        ok: bool,
        envelope: Vec<u8>,
    },
    /// 3, server → DLL: the callback target (0 = the game window), event code, envelope.
    Event {
        target: u32,
        code: u32,
        envelope: Vec<u8>,
    },
}

/// The largest frame payload (an envelope of `protocol::nm::MAX_ENVELOPE` plus its fields).
pub const MAX_FRAME: usize = crate::protocol::nm::MAX_ENVELOPE + 64;

impl Frame {
    pub fn encode(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        let (kind, a, b, envelope): (u8, u32, u32, &[u8]) = match self {
            Self::Call {
                seq,
                code,
                envelope,
            } => (1, *seq, *code, envelope),
            Self::Reply { seq, ok, envelope } => (2, *seq, u32::from(*ok), envelope),
            Self::Event {
                target,
                code,
                envelope,
            } => (3, *target, *code, envelope),
        };
        payload.push(kind);
        payload.extend_from_slice(&a.to_le_bytes());
        if kind == 2 {
            payload.push(b as u8);
        } else {
            payload.extend_from_slice(&b.to_le_bytes());
        }
        payload.extend_from_slice(envelope);
        let mut out = (payload.len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(&payload);
        out
    }
    /// A frame's payload (after its length prefix).
    pub fn decode(payload: &[u8]) -> Option<Self> {
        let (&kind, rest) = payload.split_first()?;
        let word = |b: &[u8], at: usize| {
            b.get(at..at + 4)
                .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        };
        let a = word(rest, 0)?;
        match kind {
            1 => Some(Self::Call {
                seq: a,
                code: word(rest, 4)?,
                envelope: rest.get(8..)?.to_vec(),
            }),
            2 => Some(Self::Reply {
                seq: a,
                ok: *rest.get(4)? != 0,
                envelope: rest.get(5..)?.to_vec(),
            }),
            3 => Some(Self::Event {
                target: a,
                code: word(rest, 4)?,
                envelope: rest.get(8..)?.to_vec(),
            }),
            _ => None,
        }
    }
}

/// NMCO function codes the service answers.
pub mod code {
    pub const REGISTER_CALLBACK: u32 = 0x1002;
    pub const LOGIN_MESSENGER: u32 = 0x110d;
    pub const LOGOUT_MESSENGER: u32 = 0x110e;
    pub const LOGIN_AUTH: u32 = 0x110f;
    pub const INITIALIZE: u32 = 0x1111;
    pub const SET_LOCALE: u32 = 0x2104;
    pub const SET_CODE_PAGE: u32 = 0x2109;
    pub const GET_FRIEND_LIST: u32 = 0x4001;
    /// Event MessengerReply.
    pub const EVENT_MESSENGER_REPLY: u32 = 0x1001;
}

/// One messenger connection's state.
#[derive(Default)]
pub struct Session {
    pub account: Option<Account>,
    pub online: bool,
}

/// What a call does: its reply and the events to push after it.
pub struct Answer {
    pub ok: bool,
    pub envelope: Vec<u8>,
    pub events: Vec<(u32, u32, Vec<u8>)>,
}

fn answer(ok: bool) -> Answer {
    use crate::protocol::nm;
    Answer {
        ok,
        envelope: nm::reply(ok, &nm::Writer::default()),
        events: Vec::new(),
    }
}

/// The calls answered without a server-side record: callbacks, Initialize, locale, code page,
/// LoginAuth with a known passport, LoginMessenger (MessengerReply 0 to the game window) and
/// an empty friend list. Others fail (benign: the engine treats them as a failed call).
pub fn handle(session: &mut Session, passports: &Passports, code: u32, envelope: &[u8], now: Instant) -> Answer {
    use crate::protocol::nm;
    match code {
        code::REGISTER_CALLBACK | code::INITIALIZE | code::SET_LOCALE | code::SET_CODE_PAGE => answer(true),
        code::LOGIN_AUTH => {
            let passport = nm::open(envelope)
                .ok()
                .and_then(|body| {
                    let mut call = nm::call(&body).ok()?;
                    call.fields.wstring().ok()
                });
            session.account = passport.and_then(|p| passports.account(&p, now));
            answer(session.account.is_some())
        }
        code::LOGIN_MESSENGER if session.account.is_some() => {
            session.online = true;
            let mut fields = nm::Writer::default();
            fields.u32(0);
            Answer {
                ok: true,
                envelope: nm::reply(true, &nm::Writer::default()),
                events: vec![(0, code::EVENT_MESSENGER_REPLY, nm::event(&fields))],
            }
        }
        code::LOGOUT_MESSENGER => {
            session.online = false;
            answer(true)
        }
        code::GET_FRIEND_LIST if session.online => {
            // bSuccess, no categories, no friends.
            let mut results = nm::Writer::default();
            results.u32(0).u32(0);
            Answer {
                ok: true,
                envelope: nm::reply(true, &results),
                events: Vec::new(),
            }
        }
        _ => answer(false),
    }
}

#[cfg(test)]
mod service_tests {
    use super::*;
    use crate::protocol::nm;

    fn login_auth(passport: &str) -> Vec<u8> {
        let mut fields = nm::Writer::default();
        fields.u32(0).wstring(passport, 0x1000).wstring("id", 0x100).wstring("", 0x80);
        let mut body = nm::Writer::default();
        body.object(nm::FUNC, 1, &fields);
        nm::seal_v1(&body.bytes, 7)
    }

    #[test]
    fn frames_round_trip() {
        for f in [
            Frame::Call { seq: 1, code: 0x110f, envelope: vec![1, 2] },
            Frame::Reply { seq: 9, ok: true, envelope: vec![] },
            Frame::Event { target: 0, code: 0x1001, envelope: vec![3] },
        ] {
            let bytes = f.encode();
            assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize, bytes.len() - 4);
            assert_eq!(Frame::decode(&bytes[4..]), Some(f));
        }
    }

    #[test]
    fn login_needs_a_known_passport_then_replies_and_lists() {
        let passports = Passports::default();
        let now = Instant::now();
        passports.remember("ticket", Account { nexon_sn: 31, nexon_id: "owner".to_owned() }, now);
        let mut s = Session::default();
        assert!(!handle(&mut s, &passports, code::LOGIN_AUTH, &login_auth("wrong"), now).ok);
        assert!(!handle(&mut s, &passports, code::LOGIN_MESSENGER, &[], now).ok);
        assert!(handle(&mut s, &passports, code::LOGIN_AUTH, &login_auth("ticket"), now).ok);
        let login = handle(&mut s, &passports, code::LOGIN_MESSENGER, &[], now);
        assert!(login.ok);
        assert_eq!(login.events[0].1, code::EVENT_MESSENGER_REPLY);
        let list = handle(&mut s, &passports, code::GET_FRIEND_LIST, &[], now);
        assert!(list.ok);
        assert_eq!(nm::open(&list.envelope).unwrap().len(), 8 + 12);
        assert!(!handle(&mut s, &passports, 0x6001, &[], now).ok);
    }
}
