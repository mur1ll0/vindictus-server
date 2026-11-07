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
    pub const GET_MY_INFO: u32 = 0x2001;
    pub const REQUEST_NEW_FRIEND: u32 = 0x4003;
    pub const CONFIRM_NEW_FRIEND: u32 = 0x4004;
    pub const BLOCK_FRIEND: u32 = 0x4005;
    pub const ADD_FRIEND_TO_CATEGORY: u32 = 0x4007;
    pub const DELETE_FRIEND_FROM_CATEGORY: u32 = 0x4008;
    pub const MOVE_FRIEND_CATEGORY: u32 = 0x4009;
    pub const ADD_CATEGORY: u32 = 0x4101;
    pub const DELETE_CATEGORY: u32 = 0x4102;
    pub const CHANGE_CATEGORY_NAME: u32 = 0x4103;
    pub const GET_GAME_SERVER_NAME: u32 = 0x8011;
    /// Event MessengerReply.
    pub const EVENT_MESSENGER_REPLY: u32 = 0x1001;
    /// Event RequestNewFriend (CNMRequestNewFriendData).
    pub const EVENT_REQUEST_NEW_FRIEND: u32 = 0x1005;
    /// Event FriendInfoChanged (CNMFriendInfo, u8 avatar follows).
    pub const EVENT_FRIEND_INFO_CHANGED: u32 = 0x1010;
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

/// The laboratory messenger listener (published like the relay; `nmcogame.ini` points at it).
pub const BIND: std::net::SocketAddr =
    std::net::SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 47611);
/// Concurrent messenger connections.
const MAX_CONNECTIONS: usize = 256;
/// A connection without a frame for this long is closed.
const IDLE: Duration = Duration::from_secs(600);

/// Accepts tunnel connections: each frame of kind Call gets its Reply, then its events.
pub async fn serve(listener: tokio::net::TcpListener, service: std::sync::Arc<Service>) {
    let limit = std::sync::Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            continue;
        };
        let Ok(permit) = limit.clone().try_acquire_owned() else {
            continue;
        };
        let service = service.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if connection(stream, &service).await.is_err() {
                tracing::info!("messenger connection closed; values omitted");
            }
        });
    }
}

async fn connection(stream: tokio::net::TcpStream, service: &Service) -> std::io::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let (mut read, mut write) = stream.into_split();
    let (outbox, mut queued) = tokio::sync::mpsc::channel::<Vec<u8>>(OUTBOX);
    let writer = tokio::spawn(async move {
        while let Some(bytes) = queued.recv().await {
            if write.write_all(&bytes).await.is_err() {
                break;
            }
        }
    });
    let context = std::sync::Arc::new(Context::default());
    let mut session = Session::default();
    let mut joined: Option<(i32, u64)> = None;
    let result: std::io::Result<()> = async {
        loop {
            let mut len = [0u8; 4];
            tokio::time::timeout(IDLE, read.read_exact(&mut len))
                .await
                .map_err(|_| std::io::ErrorKind::TimedOut)??;
            let len = u32::from_le_bytes(len) as usize;
            if len == 0 || len > MAX_FRAME {
                return Err(std::io::ErrorKind::InvalidData.into());
            }
            let mut payload = vec![0u8; len];
            read.read_exact(&mut payload).await?;
            let Some(Frame::Call { seq, code, envelope }) = Frame::decode(&payload) else {
                return Err(std::io::ErrorKind::InvalidData.into());
            };
            let a = service.call(&mut session, &context, code, &envelope).await;
            tracing::info!(code, ok = a.ok, "messenger call processed; values omitted");
            let mut out = Frame::Reply { seq, ok: a.ok, envelope: a.envelope }.encode();
            for (target, code, envelope) in a.events {
                out.extend(Frame::Event { target, code, envelope }.encode());
            }
            outbox
                .send(out)
                .await
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::BrokenPipe))?;
            let me = session.account.as_ref().map(|a| a.nexon_sn);
            match (session.online, joined, me) {
                (true, None, Some(sn)) => {
                    joined = Some((sn, service.hub.join(sn, outbox.clone(), context.clone())));
                    service.presence(sn).await;
                }
                (false, Some((sn, id)), _) => {
                    joined = None;
                    if service.hub.leave(sn, id) {
                        service.presence(sn).await;
                    }
                }
                _ => {}
            }
        }
    }
    .await;
    if let Some((sn, id)) = joined
        && service.hub.leave(sn, id)
    {
        service.presence(sn).await;
    }
    drop(outbox);
    let _ = writer.await;
    result
}

/// What the engine of one connection reveals: its game code (the GetGameServerName argument)
/// and virtual id. Events about friends carry them; the engine drops those that do not match.
#[derive(Default)]
pub struct Context {
    pub game_code: std::sync::atomic::AtomicU32,
    pub virtual_id: std::sync::atomic::AtomicU32,
}
impl Context {
    fn pair(&self) -> (u32, u32) {
        use std::sync::atomic::Ordering::Relaxed;
        (self.game_code.load(Relaxed), self.virtual_id.load(Relaxed))
    }
}

/// The game server name GetGameServerName returns; friend names carry it as "[name]".
pub const SERVER_NAME: &str = "Vindictus";
/// The name of the default category (0).
pub const DEFAULT_CATEGORY: &str = "Amigos";
/// ConfirmNewFriend code that accepts (others refuse) [I, docs/messenger-protocol.md].
const CONFIRM_ACCEPT: u8 = 1;

type Error = super::messenger_friends::FriendError;

/// The messenger with its database and online connections.
pub struct Service {
    pub passports: std::sync::Arc<Passports>,
    pub pool: Option<sqlx::PgPool>,
    pub hub: MessengerHub,
}

/// The list name of an account: "[server]" + its character (or login).
pub fn display(p: &super::messenger_friends::Profile) -> String {
    let name = if p.character.is_empty() { &p.login } else { &p.character };
    format!("[{SERVER_NAME}]{name}")
}

fn ok_with(results: &crate::protocol::nm::Writer) -> Answer {
    Answer {
        ok: true,
        envelope: crate::protocol::nm::reply(true, results),
        events: Vec::new(),
    }
}

/// A friend entry as `owner` (game code, virtual id) sees it.
fn view<'a>(
    p: &'a super::messenger_friends::Profile,
    name: &'a str,
    memo: &'a str,
    online: bool,
    owner: (u32, u32),
) -> crate::protocol::nm::objects::FriendView<'a> {
    use crate::protocol::nm::objects::{FriendKey, FriendView, User};
    let id_code = u64::from(p.nexon_sn.unsigned_abs());
    FriendView {
        user: User { login: &p.login, id_code, nickname: &p.character, online },
        key: FriendKey { id_code, owner, friend: (owner.0, 0) },
        display: name,
        memo,
    }
}

impl Service {
    pub fn new(passports: std::sync::Arc<Passports>, pool: Option<sqlx::PgPool>) -> Self {
        Self { passports, pool, hub: MessengerHub::default() }
    }

    /// One call: the friend functions with the database, the rest by `handle`.
    pub async fn call(&self, s: &mut Session, context: &Context, code: u32, envelope: &[u8]) -> Answer {
        use crate::protocol::nm;
        let body = nm::open(envelope).ok();
        let call = body.as_deref().and_then(|b| nm::call(b).ok());
        if code == code::GET_GAME_SERVER_NAME {
            if let Some(mut c) = call {
                let game_code = c.fields.u32().unwrap_or(c.argument);
                context.game_code.store(game_code, std::sync::atomic::Ordering::Relaxed);
            }
            let mut results = nm::Writer::default();
            results.wstring(SERVER_NAME, 0x7f);
            return ok_with(&results);
        }
        let me = s.account.as_ref().filter(|_| s.online).map(|a| a.nexon_sn);
        let (Some(pool), Some(me)) = (self.pool.as_ref(), me) else {
            return handle(s, &self.passports, code, envelope, Instant::now());
        };
        let Some(mut c) = call else {
            return handle(s, &self.passports, code, envelope, Instant::now());
        };
        let outcome = match code {
            code::GET_MY_INFO => self.my_info(pool, me).await,
            code::GET_FRIEND_LIST => self.friend_list(pool, me, context).await,
            code::REQUEST_NEW_FRIEND => self.request(pool, me, &mut c.fields).await,
            code::CONFIRM_NEW_FRIEND => self.confirm(pool, me, &mut c.fields).await,
            code::BLOCK_FRIEND
            | code::ADD_FRIEND_TO_CATEGORY
            | code::DELETE_FRIEND_FROM_CATEGORY
            | code::MOVE_FRIEND_CATEGORY
            | code::ADD_CATEGORY
            | code::DELETE_CATEGORY
            | code::CHANGE_CATEGORY_NAME => self.edit(pool, me, code, &mut c.fields).await,
            _ => return handle(s, &self.passports, code, envelope, Instant::now()),
        };
        outcome.unwrap_or_else(|_| {
            tracing::info!(code, "messenger call refused; values omitted");
            answer(false)
        })
    }

    async fn my_info(&self, pool: &sqlx::PgPool, me: i32) -> Result<Answer, Error> {
        use crate::protocol::nm::{Writer, objects};
        let p = super::messenger_friends::profile(pool, me).await?.ok_or(Error::Refused)?;
        let mut results = Writer::default();
        let user = objects::User {
            login: &p.login,
            id_code: u64::from(me.unsigned_abs()),
            nickname: &p.character,
            online: true,
        };
        objects::real_user(&mut results, &user);
        results.u64(0).u64(0).u32(0).u32(0).wstring("", 0xfff);
        Ok(ok_with(&results))
    }

    /// The categories (0 first) with their friends, then every friend; requests waiting for an
    /// answer follow as RequestNewFriend events.
    async fn friend_list(&self, pool: &sqlx::PgPool, me: i32, context: &Context) -> Result<Answer, Error> {
        use super::messenger_friends::{self as db, State};
        use crate::protocol::nm::{self, Writer, objects};
        let mine = context.pair();
        let categories = db::categories(pool, me).await?;
        let all = db::friends(pool, me).await?;
        let shown: Vec<_> = all.iter().filter(|f| f.state == State::Friend).collect();
        let names: Vec<String> = shown.iter().map(|f| display(&f.profile)).collect();
        let online: Vec<bool> = shown.iter().map(|f| self.hub.is_online(f.profile.nexon_sn)).collect();
        let in_category = |id: i32| -> Vec<objects::FriendView<'_>> {
            (0..shown.len())
                .filter(|&i| shown[i].category_id == id)
                .map(|i| view(&shown[i].profile, &names[i], &shown[i].memo, online[i], mine))
                .collect()
        };
        let mut results = Writer::default();
        results.u32(categories.len() as u32 + 1);
        objects::category(&mut results, 0, DEFAULT_CATEGORY, &in_category(0));
        for c in &categories {
            objects::category(&mut results, c.id as u32, &c.name, &in_category(c.id));
        }
        results.u32(shown.len() as u32);
        for i in 0..shown.len() {
            objects::friend(&mut results, &view(&shown[i].profile, &names[i], &shown[i].memo, online[i], mine));
        }
        let mut a = ok_with(&results);
        for f in all.iter().filter(|f| f.state == State::Waiting) {
            let request = objects::Request {
                serial: f.profile.nexon_sn.unsigned_abs(),
                peer: display(&f.profile),
                from: mine,
                to: mine,
                ..objects::Request::default()
            };
            let mut fields = Writer::default();
            request.write(&mut fields);
            a.events.push((0, code::EVENT_REQUEST_NEW_FRIEND, nm::event(&fields)));
        }
        Ok(a)
    }

    async fn request(
        &self,
        pool: &sqlx::PgPool,
        me: i32,
        fields: &mut crate::protocol::nm::Reader<'_>,
    ) -> Result<Answer, Error> {
        use super::messenger_friends::{self as db, Requested};
        use crate::protocol::nm::{self, objects::Request};
        let call = Request::read(fields).map_err(|_| Error::Refused)?;
        let prefix = format!("[{SERVER_NAME}]");
        let name = call.peer.strip_prefix(&prefix).unwrap_or(&call.peer);
        let target = db::find(pool, name).await?.ok_or(Error::Refused)?;
        match db::request(pool, me, target).await? {
            Requested::Sent => {
                let from = db::profile(pool, me).await?.ok_or(Error::Refused)?;
                let peer = display(&from);
                self.hub.send_with(target, code::EVENT_REQUEST_NEW_FRIEND, |pair| {
                    let request = Request {
                        serial: me.unsigned_abs(),
                        peer: peer.clone(),
                        from: pair,
                        to: pair,
                        message: call.message.clone(),
                        ..Request::default()
                    };
                    let mut w = nm::Writer::default();
                    request.write(&mut w);
                    nm::event(&w)
                });
            }
            Requested::Accepted => self.friendship_changed(pool, me, target).await?,
        }
        Ok(answer(true))
    }

    async fn confirm(
        &self,
        pool: &sqlx::PgPool,
        me: i32,
        fields: &mut crate::protocol::nm::Reader<'_>,
    ) -> Result<Answer, Error> {
        let serial = fields.u32().map_err(|_| Error::Refused)?;
        let accept = fields.u8().map_err(|_| Error::Refused)? == CONFIRM_ACCEPT;
        let other = i32::try_from(serial).map_err(|_| Error::Refused)?;
        super::messenger_friends::confirm(pool, me, other, accept).await?;
        if accept {
            self.friendship_changed(pool, me, other).await?;
        }
        Ok(answer(true))
    }

    /// Block, category membership and category edits; the client asks for the list again.
    async fn edit(
        &self,
        pool: &sqlx::PgPool,
        me: i32,
        code: u32,
        f: &mut crate::protocol::nm::Reader<'_>,
    ) -> Result<Answer, Error> {
        use super::messenger_friends as db;
        use crate::protocol::nm::{NmError, Reader, objects::FriendKey};
        fn bad(_: NmError) -> Error {
            Error::Refused
        }
        fn friend(f: &mut Reader<'_>) -> Result<i32, Error> {
            let key = FriendKey::read(f).map_err(bad)?;
            i32::try_from(key.id_code).map_err(|_| Error::Refused)
        }
        fn category(f: &mut Reader<'_>) -> Result<i32, Error> {
            i32::try_from(f.u32().map_err(bad)?).map_err(|_| Error::Refused)
        }
        match code {
            code::BLOCK_FRIEND => {
                let other = friend(f)?;
                let block = f.u8().unwrap_or(1) != 0;
                db::block(pool, me, other, block).await?;
            }
            code::ADD_FRIEND_TO_CATEGORY => {
                let other = friend(f)?;
                db::move_friend(pool, me, other, category(f)?).await?;
            }
            code::DELETE_FRIEND_FROM_CATEGORY => {
                // A friend belongs to one category: leaving it ends the friendship.
                let other = friend(f)?;
                db::remove(pool, me, other).await?;
                self.send_friend(pool, other, me, false).await?;
            }
            code::MOVE_FRIEND_CATEGORY => {
                let other = friend(f)?;
                category(f)?;
                db::move_friend(pool, me, other, category(f)?).await?;
            }
            code::ADD_CATEGORY => {
                f.u32().map_err(bad)?;
                f.u32().map_err(bad)?;
                let name = f.wstring().map_err(bad)?;
                db::add_category(pool, me, &name).await?;
            }
            code::DELETE_CATEGORY => db::delete_category(pool, me, category(f)?).await?,
            _ => {
                let id = category(f)?;
                let name = f.wstring().map_err(bad)?;
                db::rename_category(pool, me, id, &name).await?;
            }
        }
        Ok(answer(true))
    }

    /// FriendInfoChanged 0x1010 to `to` about `about`, online as the hub sees it (`listed`
    /// false sends it offline, the closest the protocol has to a removal).
    async fn send_friend(&self, pool: &sqlx::PgPool, to: i32, about: i32, listed: bool) -> Result<(), Error> {
        use crate::protocol::nm::{self, Writer, objects};
        if !self.hub.is_online(to) {
            return Ok(());
        }
        let Some(p) = super::messenger_friends::profile(pool, about).await? else {
            return Ok(());
        };
        let online = listed && self.hub.is_online(about);
        let name = display(&p);
        self.hub.send_with(to, code::EVENT_FRIEND_INFO_CHANGED, |pair| {
            let mut w = Writer::default();
            objects::friend(&mut w, &view(&p, &name, "", online, pair));
            w.u8(0);
            nm::event(&w)
        });
        Ok(())
    }

    async fn friendship_changed(&self, pool: &sqlx::PgPool, a: i32, b: i32) -> Result<(), Error> {
        self.send_friend(pool, a, b, true).await?;
        self.send_friend(pool, b, a, true).await
    }

    /// An account came online or went offline: its friends online hear it (the single point
    /// of presence).
    pub async fn presence(&self, me: i32) {
        use super::messenger_friends::{self as db, State};
        let Some(pool) = self.pool.as_ref() else {
            return;
        };
        let Ok(friends) = db::friends(pool, me).await else {
            return;
        };
        for f in friends.iter().filter(|f| f.state == State::Friend) {
            if self.send_friend(pool, f.profile.nexon_sn, me, true).await.is_err() {
                tracing::info!("messenger presence not sent; values omitted");
            }
        }
    }
}

#[cfg(test)]
mod listener_tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn a_call_frame_gets_its_reply_frame() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let service = Service::new(std::sync::Arc::new(Passports::default()), None);
        let server = tokio::spawn(serve(listener, std::sync::Arc::new(service)));
        let mut stream = tokio::net::TcpStream::connect(address).await.unwrap();
        let call = Frame::Call { seq: 5, code: code::INITIALIZE, envelope: Vec::new() }.encode();
        stream.write_all(&call).await.unwrap();
        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.unwrap();
        let mut payload = vec![0u8; u32::from_le_bytes(len) as usize];
        stream.read_exact(&mut payload).await.unwrap();
        let Some(Frame::Reply { seq, ok, .. }) = Frame::decode(&payload) else {
            panic!("expected a reply");
        };
        assert_eq!((seq, ok), (5, true));
        stream.write_all(&[0, 0, 0, 0]).await.unwrap();
        assert_eq!(stream.read(&mut len).await.unwrap(), 0);
        server.abort();
    }
}

/// Frames queued per connection before further events to it are dropped.
const OUTBOX: usize = 256;

/// The online messenger connections per account: the single point through which a change seen
/// by other players (presence, requests, friend changes, chat) reaches them.
#[derive(Default)]
pub struct MessengerHub {
    online: Mutex<HashMap<i32, Vec<Connection>>>,
    next: std::sync::atomic::AtomicU64,
}
struct Connection {
    id: u64,
    outbox: tokio::sync::mpsc::Sender<Vec<u8>>,
    context: std::sync::Arc<Context>,
}
impl MessengerHub {
    /// A connection of `nexon_sn` came online; the returned id removes it again.
    pub fn join(
        &self,
        nexon_sn: i32,
        outbox: tokio::sync::mpsc::Sender<Vec<u8>>,
        context: std::sync::Arc<Context>,
    ) -> u64 {
        let id = self.next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut online = self.online.lock().unwrap_or_else(PoisonError::into_inner);
        online.entry(nexon_sn).or_default().push(Connection { id, outbox, context });
        id
    }
    /// Removes a connection; true when the account has no other connection left.
    pub fn leave(&self, nexon_sn: i32, id: u64) -> bool {
        let mut online = self.online.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(list) = online.get_mut(&nexon_sn) else {
            return false;
        };
        list.retain(|c| c.id != id);
        if list.is_empty() {
            online.remove(&nexon_sn);
            true
        } else {
            false
        }
    }
    pub fn is_online(&self, nexon_sn: i32) -> bool {
        let online = self.online.lock().unwrap_or_else(PoisonError::into_inner);
        online.contains_key(&nexon_sn)
    }
    /// Pushes an event to the game window (target 0) of every connection of an account, built
    /// with that connection's (game code, virtual id); a full outbox drops it.
    pub fn send_with(&self, nexon_sn: i32, code: u32, build: impl Fn((u32, u32)) -> Vec<u8>) {
        let online = self.online.lock().unwrap_or_else(PoisonError::into_inner);
        for c in online.get(&nexon_sn).into_iter().flatten() {
            let envelope = build(c.context.pair());
            let frame = Frame::Event { target: 0, code, envelope }.encode();
            let _ = c.outbox.try_send(frame);
        }
    }
}
