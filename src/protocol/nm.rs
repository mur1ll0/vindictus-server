//! The Nexon Messenger call format of the 1.69 client (B5 MS3a, decision D-S1;
//! docs/messenger-protocol.md): the stream envelope (v0 plain, v1 XOR-chained and reversed)
//! that `NMCO_CallNMFunction` carries, and the CNMSerializable encoding inside it (object header
//! `typeWord << 16 | b6 << 8 | version`, body length, little-endian primitives, compact lengths
//! and UTF-16LE strings). The laboratory `nmcogame.dll` tunnels these envelopes to the server.
use thiserror::Error;

/// CStreamCryptor_v0.
pub const V0_MAGIC: u32 = 0x001F_CA34;
pub const V0_TRAILER: u32 = 0x008A_119E;
/// CStreamCryptor_v1 (what the engine sends).
pub const V1_MAGIC: u32 = 0x0A98_C32A;
pub const V1_TRAILER: u32 = 0x820A_9E12;
/// Type words of CNMFunc ('fo') and CNMEvent ('eo').
pub const FUNC: u16 = 0x666f;
pub const EVENT: u16 = 0x656f;
/// Largest envelope accepted (the engine's reply buffer is 1 MB).
pub const MAX_ENVELOPE: usize = 0x10_0000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NmError {
    #[error("truncated messenger data")]
    Truncated,
    #[error("unknown envelope")]
    Envelope,
    #[error("messenger data too large")]
    TooLarge,
    #[error("unexpected object type")]
    Object,
    #[error("invalid string")]
    Text,
}

/// The body of an envelope (v0 plain or v1 deobfuscated; the v1 checksum is not checked).
pub fn open(envelope: &[u8]) -> Result<Vec<u8>, NmError> {
    if envelope.len() > MAX_ENVELOPE + 12 {
        return Err(NmError::TooLarge);
    }
    let word = |at: usize| {
        envelope
            .get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or(NmError::Truncated)
    };
    let (magic, len) = (word(0)?, word(4)? as usize);
    let body = envelope.get(8..8 + len).ok_or(NmError::Truncated)?;
    let trailer = word(8 + len)?;
    match (magic, trailer) {
        (V0_MAGIC, V0_TRAILER) => Ok(body.to_vec()),
        (V1_MAGIC, V1_TRAILER) => {
            // out = reverse(tmp) ++ u16 checksum; tmp = 8 header bytes ++ chained bytes.
            if body.len() < 10 {
                return Err(NmError::Truncated);
            }
            let tmp: Vec<u8> = body[..body.len() - 2].iter().rev().copied().collect();
            if tmp[1] != 1 {
                return Err(NmError::Envelope);
            }
            let key = tmp[3];
            Ok((8..tmp.len())
                .map(|i| tmp[i] ^ key ^ if i == 8 { 0 } else { tmp[i - 1] })
                .collect())
        }
        _ => Err(NmError::Envelope),
    }
}

/// A v0 envelope of a body (the replies and events of the laboratory).
pub fn seal(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(&V0_MAGIC.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out.extend_from_slice(&V0_TRAILER.to_le_bytes());
    out
}

/// A v1 envelope (the engine's own codec; used by the tests to mirror its requests). The
/// checksum is the engine's 16-bit fold, which `open` does not check; zero here.
pub fn seal_v1(body: &[u8], key: u32) -> Vec<u8> {
    let mut tmp = vec![0x5a, 1, 0xa5];
    tmp.extend_from_slice(&key.to_le_bytes());
    tmp.push(0x3c);
    let k = key as u8;
    for (i, b) in body.iter().enumerate() {
        let prev = if i == 0 { 0 } else { tmp[8 + i - 1] };
        tmp.push(b ^ k ^ prev);
    }
    let mut out: Vec<u8> = tmp.into_iter().rev().collect();
    out.extend_from_slice(&[0, 0]);
    let mut env = Vec::with_capacity(out.len() + 12);
    env.extend_from_slice(&V1_MAGIC.to_le_bytes());
    env.extend_from_slice(&(out.len() as u32).to_le_bytes());
    env.extend_from_slice(&out);
    env.extend_from_slice(&V1_TRAILER.to_le_bytes());
    env
}

/// CNMSerializableDecoder over a body.
pub struct Reader<'a> {
    rest: &'a [u8],
}
impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }
    pub fn remaining(&self) -> usize {
        self.rest.len()
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], NmError> {
        if self.rest.len() < n {
            return Err(NmError::Truncated);
        }
        let (a, b) = self.rest.split_at(n);
        self.rest = b;
        Ok(a)
    }
    pub fn u8(&mut self) -> Result<u8, NmError> {
        Ok(self.take(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16, NmError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    pub fn u32(&mut self) -> Result<u32, NmError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    pub fn u64(&mut self) -> Result<u64, NmError> {
        let b = self.take(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(b);
        Ok(u64::from_le_bytes(a))
    }
    /// Compact length: the low two bits of the first byte select u8, u16, u32 (value >> 2) or a
    /// tag byte followed by a u32.
    pub fn length(&mut self) -> Result<usize, NmError> {
        let first = self.rest.first().copied().ok_or(NmError::Truncated)?;
        let n = match first & 3 {
            0 => u32::from(self.u8()? >> 2),
            1 => u32::from(self.u16()? >> 2),
            2 => self.u32()? >> 2,
            _ => {
                self.u8()?;
                self.u32()?
            }
        };
        usize::try_from(n).map_err(|_| NmError::TooLarge)
    }
    /// A wstring: compact length in UTF-16 units, UTF-16LE, no terminator.
    pub fn wstring(&mut self) -> Result<String, NmError> {
        let units = self.length()?;
        if units > MAX_ENVELOPE / 2 {
            return Err(NmError::TooLarge);
        }
        let raw = self.take(units * 2)?;
        let wide: Vec<u16> = raw
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        String::from_utf16(&wide).map_err(|_| NmError::Text)
    }
    /// An object header: (typeWord, b6, version) and the body as its own reader.
    pub fn object(&mut self) -> Result<(u16, u8, u8, Reader<'a>), NmError> {
        let header = self.u32()?;
        let len = self.u32()? as usize;
        let body = self.take(len)?;
        Ok((
            (header >> 16) as u16,
            (header >> 8) as u8,
            header as u8,
            Reader::new(body),
        ))
    }
}

/// CNMSerializableEncoder into a body.
#[derive(Default)]
pub struct Writer {
    pub bytes: Vec<u8>,
}
impl Writer {
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.bytes.push(v);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.bytes.extend_from_slice(&v.to_le_bytes());
        self
    }
    /// The smallest compact length form.
    pub fn length(&mut self, n: usize) -> &mut Self {
        let n = n as u32;
        if n < 0x40 {
            self.u8((n << 2) as u8)
        } else if n < 0x4000 {
            self.u16(((n << 2) | 1) as u16)
        } else if n < 0x4000_0000 {
            self.u32((n << 2) | 2)
        } else {
            self.u8(3).u32(n)
        }
    }
    /// A wstring cut to `max` UTF-16 units (the reader's field size).
    pub fn wstring(&mut self, text: &str, max: usize) -> &mut Self {
        let units: Vec<u16> = text.encode_utf16().take(max).collect();
        self.length(units.len());
        for u in units {
            self.u16(u);
        }
        self
    }
    /// A nested object: header and body length, then `body`'s bytes.
    pub fn object(&mut self, type_word: u16, b6: u8, body: &Writer) -> &mut Self {
        self.u32((u32::from(type_word) << 16) | (u32::from(b6) << 8) | 1);
        self.u32(body.bytes.len() as u32);
        self.bytes.extend_from_slice(&body.bytes);
        self
    }
}

/// A function call of the engine: code, the call argument and the derived fields.
pub struct Call<'a> {
    pub argument: u32,
    pub fields: Reader<'a>,
}

/// Opens a function call body (a 'fo' object, version 1).
pub fn call(body: &[u8]) -> Result<Call<'_>, NmError> {
    let mut r = Reader::new(body);
    let (word, _, _, mut fields) = r.object()?;
    if word != FUNC {
        return Err(NmError::Object);
    }
    let argument = fields.u32()?;
    Ok(Call { argument, fields })
}

/// A function reply envelope: `bSuccess` then the result fields.
pub fn reply(success: bool, results: &Writer) -> Vec<u8> {
    let mut body = Writer::default();
    body.u32(u32::from(success));
    body.bytes.extend_from_slice(&results.bytes);
    let mut object = Writer::default();
    object.object(FUNC, 1, &body);
    seal(&object.bytes)
}

/// An event envelope ('eo' object) with its fields.
pub fn event(fields: &Writer) -> Vec<u8> {
    let mut object = Writer::default();
    object.object(EVENT, 1, fields);
    seal(&object.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v1_requests_open_and_v0_replies_round_trip() {
        let mut fields = Writer::default();
        fields.u32(0).u32(0x1234).u32(0x8001);
        let mut body = Writer::default();
        body.object(FUNC, 1, &fields);
        let envelope = seal_v1(&body.bytes, 0xdead_beef);
        let opened = open(&envelope).unwrap();
        assert_eq!(opened, body.bytes);
        let mut c = call(&opened).unwrap();
        assert_eq!((c.argument, c.fields.u32().unwrap(), c.fields.u32().unwrap()), (0, 0x1234, 0x8001));
        let r = reply(true, &Writer::default());
        assert_eq!(
            r,
            [
                0x34, 0xca, 0x1f, 0x00, 12, 0, 0, 0, 0x01, 0x01, 0x6f, 0x66, 4, 0, 0, 0, 1, 0, 0,
                0, 0x9e, 0x11, 0x8a, 0x00
            ]
        );
        assert_eq!(open(&r).unwrap().len(), 12);
        assert_eq!(open(&[1, 2, 3]), Err(NmError::Truncated));
    }

    #[test]
    fn compact_lengths_and_wstrings() {
        let mut w = Writer::default();
        w.wstring("Fiona_é", 0x40).length(0x3f).length(0x40).length(0x4000).length(0x4000_0000);
        let mut r = Reader::new(&w.bytes);
        assert_eq!(r.wstring().unwrap(), "Fiona_é");
        assert_eq!(r.length().unwrap(), 0x3f);
        assert_eq!(r.length().unwrap(), 0x40);
        assert_eq!(r.length().unwrap(), 0x4000);
        assert_eq!(r.length().unwrap(), 0x4000_0000);
        assert_eq!(r.remaining(), 0);
        let mut cut = Writer::default();
        cut.wstring("abcdef", 3);
        assert_eq!(Reader::new(&cut.bytes).wstring().unwrap(), "abc");
    }
}

/// The messenger data objects the engine decodes (B5 MS3b; .cache/reverse notes summarized in
/// docs/messenger-protocol.md §2.4). A derived class keeps its base's type word and only raises
/// b6; every object is written flat (version 1). Fields whose meaning is unknown are zero, which
/// the engine accepts.
pub mod objects {
    use super::{Reader, NmError, Writer};

    pub const USER: u16 = 0x7569;
    pub const CATEGORY: u16 = 0x6369;
    pub const REQUEST: u16 = 0x7266;
    pub const AVATAR: u16 = 0x6169;
    pub const CHAT_MESSAGE: u16 = 0x636d;
    pub const SESSION: u16 = 0x7369;
    pub const SESSION_MEMBER: u16 = 0x736d;
    /// DefaultUserInfo status of an online user (0, 13 and 14 are offline).
    pub const ONLINE: u32 = 1;

    /// A user as the objects show it.
    pub struct User<'a> {
        pub login: &'a str,
        pub id_code: u64,
        pub nickname: &'a str,
        pub online: bool,
    }

    /// NMFriendKey: the friend's IDCode, the owner's (game code, virtual id), the friend's.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct FriendKey {
        pub id_code: u64,
        pub owner: (u32, u32),
        pub friend: (u32, u32),
    }
    impl FriendKey {
        pub fn write(&self, w: &mut Writer) {
            w.u64(self.id_code)
                .u32(self.owner.0)
                .u32(self.owner.1)
                .u32(self.friend.0)
                .u32(self.friend.1);
        }
        pub fn read(r: &mut Reader<'_>) -> Result<Self, NmError> {
            Ok(Self {
                id_code: r.u64()?,
                owner: (r.u32()?, r.u32()?),
                friend: (r.u32()?, r.u32()?),
            })
        }
    }

    /// char[N] fields hold N - 1 characters.
    fn text(w: &mut Writer, value: &str, bytes: usize) {
        w.wstring(value, bytes - 1);
    }

    /// CNMDefaultUserInfo body.
    fn default_body(w: &mut Writer, u: &User<'_>) {
        text(w, u.login, 0x40);
        w.u64(u.id_code).u32(0);
        text(w, u.nickname, 0x40);
        w.u32(if u.online { ONLINE } else { 0 }).u32(0).u32(0).u16(0);
    }

    /// CNMAvatarItemInfo (b6 1): zeros, ignored by the engine.
    pub fn avatar_item(w: &mut Writer) {
        let mut b = Writer::default();
        b.u8(0).u32(0);
        w.object(AVATAR, 1, &b);
    }

    /// CNMAvatarFullInfo (b6 2): zeros, ignored by the engine.
    pub fn avatar_full(w: &mut Writer) {
        let mut b = Writer::default();
        b.u8(0).u32(0).u8(0).u16(0).u16(0);
        for _ in 0..0x26 {
            b.u32(0).u32(0);
        }
        w.object(AVATAR, 2, &b);
    }

    /// CNMRealUserInfo (b6 2), the GetMyInfo object.
    pub fn real_user(w: &mut Writer, u: &User<'_>) {
        let mut b = Writer::default();
        default_body(&mut b, u);
        b.wstring("", 0x1f).wstring("", 0x3f).u32(0).u8(0).u32(0).u32(0);
        b.wstring("", 1).u32(0).wstring("", 1).u32(0).u8(0).u8(0);
        avatar_full(&mut b);
        b.u8(0).u8(0).u8(0).u8(0).u8(0).wstring("", 0x3f);
        w.object(USER, 2, &b);
    }

    /// A friend as the list shows it.
    pub struct FriendView<'a> {
        pub user: User<'a>,
        pub key: FriendKey,
        /// "[" + game server name + "]" + name: the list hides other entries.
        pub display: &'a str,
        pub memo: &'a str,
    }

    /// CNMFriendInfo (b6 3).
    pub fn friend(w: &mut Writer, f: &FriendView<'_>) {
        let mut b = Writer::default();
        default_body(&mut b, &f.user);
        b.u8(0); // the discarded RealFriendInfo string
        avatar_item(&mut b);
        f.key.write(&mut b);
        text(&mut b, f.display, 0x40);
        text(&mut b, "", 0x40);
        b.u8(u8::from(f.user.online)).u32(0).u32(0).u8(0).u8(0).u8(0);
        text(&mut b, f.memo, 1000);
        text(&mut b, "", 0x20);
        w.object(USER, 3, &b);
    }

    /// CNMCateFriendInfo (b6 2): a category and its friends.
    pub fn category(w: &mut Writer, id: u32, name: &str, friends: &[FriendView<'_>]) {
        let mut b = Writer::default();
        b.u32(id).u32(0).u32(0);
        text(&mut b, name, 0x20);
        b.u32(friends.len() as u32);
        for f in friends {
            friend(&mut b, f);
        }
        b.u8(0).u8(0);
        w.object(CATEGORY, 2, &b);
    }

    /// CNMRequestNewFriendData.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Request {
        pub serial: u32,
        /// The target on the call, the requester on event 0x1005.
        pub peer: String,
        pub from: (u32, u32),
        pub to: (u32, u32),
        pub category: u32,
        pub message: String,
    }
    impl Request {
        pub fn write(&self, w: &mut Writer) {
            let mut b = Writer::default();
            b.u32(self.serial);
            text(&mut b, &self.peer, 0x40);
            b.u32(0).u32(self.from.0).u32(self.from.1).u32(self.to.0).u32(self.to.1);
            text(&mut b, "", 0x40);
            text(&mut b, "", 0x40);
            b.u32(self.category);
            text(&mut b, &self.message, 0x100);
            b.u8(0);
            w.object(REQUEST, 1, &b);
        }
        pub fn read(r: &mut Reader<'_>) -> Result<Self, NmError> {
            let (word, _, _, mut b) = r.object()?;
            if word != REQUEST {
                return Err(NmError::Object);
            }
            let serial = b.u32()?;
            let peer = b.wstring()?;
            b.u32()?;
            let from = (b.u32()?, b.u32()?);
            let to = (b.u32()?, b.u32()?);
            b.wstring()?;
            b.wstring()?;
            let category = b.u32()?;
            let message = b.wstring()?;
            Ok(Self { serial, peer, from, to, category, message })
        }
    }

    /// CNMSessionInfo: session key, the receiver's (game code, virtual id); the rest zero
    /// (+0x40 = 3 would switch the sender name to a virtual user).
    pub fn session_info(w: &mut Writer, key: u32, owner: (u32, u32)) {
        let mut b = Writer::default();
        b.u32(key).u32(owner.0).u32(owner.1).u8(0).u32(0).u32(0).u64(0).u64(0);
        b.u32(0).u32(0).u32(0).u32(0);
        w.object(SESSION, 1, &b);
    }

    /// A chat session member.
    pub struct Member<'a> {
        pub id_code: u64,
        pub login: &'a str,
        pub nickname: &'a str,
        /// (game code, virtual id), passed to the client with the member.
        pub pair: (u32, u32),
        /// The name the session title shows.
        pub display: &'a str,
        pub online: bool,
    }

    /// CNMSessionMemberInfo.
    pub fn member(w: &mut Writer, m: &Member<'_>) {
        let mut b = Writer::default();
        b.u64(m.id_code);
        text(&mut b, m.login, 0x40);
        text(&mut b, m.nickname, 0x40);
        b.u32(m.pair.0).u32(m.pair.1);
        text(&mut b, m.display, 0x40);
        b.u32(if m.online { ONLINE } else { 0 });
        avatar_full(&mut b);
        w.object(SESSION_MEMBER, 1, &b);
    }

    /// CNMChatMessage: the receivers drop it unless `game_code` is theirs.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct ChatMessage {
        pub game_code: u32,
        /// "name@game short name".
        pub sender: String,
        pub text: String,
    }
    impl ChatMessage {
        pub fn write(&self, w: &mut Writer) {
            let mut b = Writer::default();
            b.u32(self.game_code).u32(0);
            avatar_item(&mut b);
            text(&mut b, &self.sender, 0x40);
            text(&mut b, "", 0x100);
            text(&mut b, "", 0x100);
            text(&mut b, &self.text, 0x400);
            b.u32(0);
            w.object(CHAT_MESSAGE, 1, &b);
        }
        pub fn read(r: &mut Reader<'_>) -> Result<Self, NmError> {
            let (word, _, _, mut b) = r.object()?;
            if word != CHAT_MESSAGE {
                return Err(NmError::Object);
            }
            let game_code = b.u32()?;
            b.u32()?;
            b.object()?;
            let sender = b.wstring()?;
            b.wstring()?;
            b.wstring()?;
            let text = b.wstring()?;
            Ok(Self { game_code, sender, text })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn objects_have_their_headers_and_requests_round_trip() {
            let mut w = Writer::default();
            avatar_full(&mut w);
            assert_eq!(w.bytes.len(), 8 + 10 + 0x26 * 8);
            assert_eq!(&w.bytes[..4], &[1, 2, 0x69, 0x61]);
            let user = User { login: "alpha", id_code: 11, nickname: "Fiona", online: true };
            let view = FriendView { user, key: FriendKey { id_code: 11, ..FriendKey::default() }, display: "[Vindictus]Fiona", memo: "" };
            let mut list = Writer::default();
            category(&mut list, 0, "Amigos", std::slice::from_ref(&view));
            let mut r = Reader::new(&list.bytes);
            let (word, b6, version, mut body) = r.object().unwrap();
            assert_eq!((word, b6, version), (CATEGORY, 2, 1));
            assert_eq!((body.u32().unwrap(), body.u32().unwrap(), body.u32().unwrap()), (0, 0, 0));
            assert_eq!(body.wstring().unwrap(), "Amigos");
            assert_eq!(body.u32().unwrap(), 1);
            let (word, b6, _, mut f) = body.object().unwrap();
            assert_eq!((word, b6), (USER, 3));
            assert_eq!(f.wstring().unwrap(), "alpha");
            assert_eq!(f.u64().unwrap(), 11);
            let request = Request { serial: 12, peer: "Beta".into(), from: (7, 0), to: (7, 0), category: 0, message: "oi".into() };
            let mut w = Writer::default();
            request.write(&mut w);
            assert_eq!(Request::read(&mut Reader::new(&w.bytes)).unwrap(), request);
            let message = ChatMessage { game_code: 7, sender: "Fiona@Vindictus".into(), text: "oi".into() };
            let mut w = Writer::default();
            message.write(&mut w);
            assert_eq!(ChatMessage::read(&mut Reader::new(&w.bytes)).unwrap(), message);
            let mut w = Writer::default();
            session_info(&mut w, 9, (7, 0));
            assert_eq!(w.bytes.len(), 8 + 4 * 3 + 1 + 8 + 16 + 16);
        }
    }
}
