//! ProudNet wire format recovered from the client's statically linked library
//! (docs/battle-transport.md): TCP framing, compact integers, message fields and the
//! messages a minimal relay server exchanges with the game client.
use thiserror::Error;

/// TCP splitter that precedes every message.
pub const SPLITTER: u16 = 0x5713;
/// Largest message the relay accepts (CNetSettings.m_serverMessageMaxLength).
pub const MAX_MESSAGE: usize = 0x10000;

/// ProudNet MessageType values used by the relay.
pub mod message_type {
    pub const RMI: u8 = 1;
    pub const USER_MESSAGE: u8 = 2;
    pub const CONNECTION_HINT: u8 = 4;
    pub const ENCRYPTED_SESSION_KEY: u8 = 5;
    pub const SESSION_KEY_SUCCESS: u8 = 6;
    pub const REQUEST_SERVER_CONNECTION: u8 = 7;
    pub const SERVER_DENIED_CONNECTION: u8 = 8;
    pub const SERVER_CONNECT_SUCCESS: u8 = 10;
    pub const RELIABLE_RELAY: u8 = 0x14;
    pub const UNRELIABLE_RELAY: u8 = 0x15;
    pub const LINGER_DATA_FRAME: u8 = 0x16;
}

/// RMI IDs: ProudNet internal (S2C/C2S) and the Heroes PIDL.
pub mod rmi {
    pub const P2P_GROUP_MEMBER_JOIN: u16 = 0xfbf5;
    pub const P2P_GROUP_MEMBER_JOIN_UNENCRYPTED: u16 = 0xfbf6;
    pub const RELIABLE_PONG: u16 = 0xfbfc;
    pub const RELIABLE_PING: u16 = 0xfa01;
    pub const P2P_GROUP_MEMBER_JOIN_ACK: u16 = 0xfa04;
    pub const SHUTDOWN_TCP_HANDSHAKE: u16 = 0xfa07;
    pub const NM_INFOS: u16 = 0x7d1;
    pub const S2C_SEND_DATA: u16 = 0x7d2;
    pub const NM_INFOS_RESULT: u16 = 0x3e9;
}

/// Heroes relay protocol version (Heroes.Managed Rmi.Start).
pub const PROTOCOL_VERSION: [u8; 16] = guid(
    0x0afa_470c,
    0x77d7,
    0x4b74,
    [0x9d, 0xdb, 0x1c, 0xb3, 0xd2, 0x5e, 0x1e, 0x63],
);

/// GUID in its native memory layout (Data1–3 little-endian, Data4 as bytes).
pub const fn guid(a: u32, b: u16, c: u16, d: [u8; 8]) -> [u8; 16] {
    let a = a.to_le_bytes();
    let b = b.to_le_bytes();
    let c = c.to_le_bytes();
    [
        a[0], a[1], a[2], a[3], b[0], b[1], c[0], c[1], d[0], d[1], d[2], d[3], d[4], d[5], d[6],
        d[7],
    ]
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WireError {
    #[error("ProudNet splitter mismatch")]
    Splitter,
    #[error("ProudNet message length out of range")]
    Length,
    #[error("ProudNet message truncated")]
    Truncated,
    #[error("ProudNet compact integer width")]
    Compact,
}

/// Appends one framed message: splitter, compact length and payload.
pub fn frame(payload: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&SPLITTER.to_le_bytes());
    put_compact(out, payload.len() as i64);
    out.extend_from_slice(payload);
}

/// CRemoteClient_S::ExtractMessageFromTcpStream: the first complete message in `buf`
/// and the bytes it used, or None while incomplete.
pub fn extract(buf: &[u8]) -> Result<Option<(Vec<u8>, usize)>, WireError> {
    if buf.len() < 2 {
        return Ok(None);
    }
    if u16::from_le_bytes([buf[0], buf[1]]) != SPLITTER {
        return Err(WireError::Splitter);
    }
    let mut r = Reader::new(&buf[2..]);
    let length = match r.compact() {
        Ok(v) => v,
        Err(WireError::Truncated) => return Ok(None),
        Err(e) => return Err(e),
    };
    if length < 0 || length as usize > MAX_MESSAGE {
        return Err(WireError::Length);
    }
    let start = 2 + r.position();
    let end = start + length as usize;
    if buf.len() < end {
        return Ok(None);
    }
    Ok(Some((buf[start..end].to_vec(), end)))
}

/// CMessage scalar: the smallest signed width (1, 2, 4 or 8 bytes) after a size byte.
pub fn put_compact(out: &mut Vec<u8>, v: i64) {
    if (-0x80..0x80).contains(&v) {
        out.push(1);
        out.push(v as i8 as u8);
    } else if (-0x8000..0x8000).contains(&v) {
        out.push(2);
        out.extend_from_slice(&(v as i16).to_le_bytes());
    } else if i32::try_from(v).is_ok() {
        out.push(4);
        out.extend_from_slice(&(v as i32).to_le_bytes());
    } else {
        out.push(8);
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Message builder with the CMessage field encodings (no test splitter).
#[derive(Default)]
pub struct Writer(pub Vec<u8>);
impl Writer {
    pub fn new(message_type: u8) -> Self {
        Self(vec![message_type])
    }
    pub fn rmi(id: u16) -> Self {
        let mut w = Self::new(message_type::RMI);
        w.u16(id);
        w
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    pub fn bool(&mut self, v: bool) -> &mut Self {
        self.0.push(u8::from(v));
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i64(&mut self, v: i64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn guid(&mut self, v: &[u8; 16]) -> &mut Self {
        self.0.extend_from_slice(v);
        self
    }
    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        put_compact(&mut self.0, v.len() as i64);
        self.0.extend_from_slice(v);
        self
    }
    /// AddrPort: the IPv4 address as stored in `s_addr` (network byte order), then
    /// the port.
    pub fn addr_port(&mut self, ip: [u8; 4], port: u16) -> &mut Self {
        self.0.extend_from_slice(&ip);
        self.u16(port)
    }
    pub fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// Bounded CMessage reader.
pub struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }
    pub fn position(&self) -> usize {
        self.at
    }
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.at
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.at.checked_add(n).ok_or(WireError::Truncated)?;
        let s = self.buf.get(self.at..end).ok_or(WireError::Truncated)?;
        self.at = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }
    pub fn bool(&mut self) -> Result<bool, WireError> {
        Ok(self.u8()? != 0)
    }
    pub fn u16(&mut self) -> Result<u16, WireError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }
    pub fn i32(&mut self) -> Result<i32, WireError> {
        let s = self.take(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }
    pub fn f32(&mut self) -> Result<f32, WireError> {
        Ok(f32::from_bits(self.i32()? as u32))
    }
    pub fn i64(&mut self) -> Result<i64, WireError> {
        let s = self.take(8)?;
        let mut a = [0; 8];
        a.copy_from_slice(s);
        Ok(i64::from_le_bytes(a))
    }
    pub fn guid(&mut self) -> Result<[u8; 16], WireError> {
        let mut a = [0; 16];
        a.copy_from_slice(self.take(16)?);
        Ok(a)
    }
    pub fn compact(&mut self) -> Result<i64, WireError> {
        Ok(match self.u8()? {
            1 => i64::from(self.u8()? as i8),
            2 => i64::from(self.u16()? as i16),
            4 => i64::from(self.i32()?),
            8 => self.i64()?,
            _ => return Err(WireError::Compact),
        })
    }
    pub fn bytes(&mut self, limit: usize) -> Result<&'a [u8], WireError> {
        let n = self.compact()?;
        if n < 0 || n as usize > limit {
            return Err(WireError::Length);
        }
        self.take(n as usize)
    }
}

/// CNetSettings sent in the connection hint, in field order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub fallback_method: u8,
    pub server_message_max_length: i32,
    pub default_timeout_sec: f64,
    pub direct_p2p_start_condition: u8,
    pub over_send_suspecting_threshold: i32,
    pub enable_nagle: bool,
    pub encrypted_message_key_length: i32,
    pub allow_server_as_p2p_group_member: bool,
    pub enable_p2p_encrypted_messaging: bool,
    pub upnp_detect_nat_device: bool,
    pub upnp_tcp_add_port_mapping: bool,
    pub emergency_log_line_count: i32,
    pub enable_lookahead_p2p_send: bool,
}
impl Settings {
    /// Relay defaults: the server answers within 60 s (RelayServer.SignonTimeOut), 128-bit
    /// RC4 (Enhanced provider), no P2P encryption and no UPnP.
    pub fn relay() -> Self {
        Self {
            fallback_method: 0,
            server_message_max_length: MAX_MESSAGE as i32,
            default_timeout_sec: 60.0,
            direct_p2p_start_condition: 0,
            over_send_suspecting_threshold: 15_000,
            enable_nagle: true,
            encrypted_message_key_length: 128,
            allow_server_as_p2p_group_member: false,
            enable_p2p_encrypted_messaging: false,
            upnp_detect_nat_device: false,
            upnp_tcp_add_port_mapping: false,
            emergency_log_line_count: 0,
            enable_lookahead_p2p_send: true,
        }
    }
}

/// Type 4: connection hint (the flag, then the settings).
pub fn connection_hint(flag: bool, s: &Settings) -> Vec<u8> {
    let mut w = Writer::new(message_type::CONNECTION_HINT);
    w.bool(flag)
        .u8(s.fallback_method)
        .i32(s.server_message_max_length)
        .f64(s.default_timeout_sec)
        .u8(s.direct_p2p_start_condition)
        .i32(s.over_send_suspecting_threshold)
        .bool(s.enable_nagle)
        .i32(s.encrypted_message_key_length)
        .bool(s.allow_server_as_p2p_group_member)
        .bool(s.enable_p2p_encrypted_messaging)
        .bool(s.upnp_detect_nat_device)
        .bool(s.upnp_tcp_add_port_mapping)
        .i32(s.emergency_log_line_count)
        .bool(s.enable_lookahead_p2p_send);
    w.finish()
}

/// Type 6: the session key encrypted for the client (CryptoAPI SIMPLEBLOB).
pub fn session_key_success(simple_blob: &[u8]) -> Vec<u8> {
    let mut w = Writer::new(message_type::SESSION_KEY_SUCCESS);
    w.bytes(simple_blob);
    w.finish()
}

/// Type 8: connection denied.
pub fn denied() -> Vec<u8> {
    vec![message_type::SERVER_DENIED_CONNECTION]
}

/// Type 10: connect success.
pub fn connect_success(
    host_id: i32,
    instance: &[u8; 16],
    reply: &[u8],
    ip: [u8; 4],
    port: u16,
) -> Vec<u8> {
    let mut w = Writer::new(message_type::SERVER_CONNECT_SUCCESS);
    w.i32(host_id)
        .guid(instance)
        .bytes(reply)
        .addr_port(ip, port);
    w.finish()
}

/// RMI 0xfbf6: P2PGroup_MemberJoin_Unencrypted.
#[allow(clippy::too_many_arguments)]
pub fn member_join_unencrypted(
    group: i32,
    member: i32,
    custom_field: &[u8],
    event_id: i32,
    first_frame_number: i32,
    magic: &[u8; 16],
    allow_direct_p2p: bool,
    port: u16,
) -> Vec<u8> {
    let mut w = Writer::rmi(rmi::P2P_GROUP_MEMBER_JOIN_UNENCRYPTED);
    w.i32(group)
        .i32(member)
        .bytes(custom_field)
        .i32(event_id)
        .i32(first_frame_number)
        .guid(magic)
        .bool(allow_direct_p2p)
        .u16(port);
    w.finish()
}

/// Heroes RMI 0x7d1: NMInfos(id, [HostID, CID, NexonSN]...).
pub fn nm_infos(id: i64, infos: &[(i32, i64, i32)]) -> Vec<u8> {
    let mut w = Writer::rmi(rmi::NM_INFOS);
    w.i64(id).i32(infos.len() as i32);
    for (host, cid, nexon_sn) in infos {
        w.i32(*host).i64(*cid).i32(*nexon_sn);
    }
    w.finish()
}

/// Client messages the relay understands.
#[derive(Debug, PartialEq)]
pub enum ClientMessage<'a> {
    /// Type 5: the client's CryptoAPI PUBLICKEYBLOB.
    PublicKey(&'a [u8]),
    /// Type 7: user data, protocol version GUID and internal version.
    RequestConnection {
        user_data: &'a [u8],
        protocol: [u8; 16],
        internal_version: i32,
    },
    ReliablePing,
    MemberJoinAck {
        group: i32,
        member: i32,
        event_id: i32,
    },
    NmInfosResult(i64),
    ShutdownTcpHandshake,
    /// Relay frames (types 20–22) and other RMIs: nothing to answer for one member.
    Ignored,
    Unknown(u8),
}

pub fn parse_client(payload: &[u8]) -> Result<ClientMessage<'_>, WireError> {
    let mut r = Reader::new(payload);
    Ok(match r.u8()? {
        message_type::ENCRYPTED_SESSION_KEY => ClientMessage::PublicKey(r.bytes(4096)?),
        message_type::REQUEST_SERVER_CONNECTION => ClientMessage::RequestConnection {
            user_data: r.bytes(4096)?,
            protocol: r.guid()?,
            internal_version: r.i32()?,
        },
        message_type::RMI => match r.u16()? {
            rmi::RELIABLE_PING => ClientMessage::ReliablePing,
            rmi::P2P_GROUP_MEMBER_JOIN_ACK => ClientMessage::MemberJoinAck {
                group: r.i32()?,
                member: r.i32()?,
                event_id: r.i32()?,
            },
            rmi::NM_INFOS_RESULT => ClientMessage::NmInfosResult(r.i64()?),
            rmi::SHUTDOWN_TCP_HANDSHAKE => ClientMessage::ShutdownTcpHandshake,
            _ => ClientMessage::Ignored,
        },
        message_type::RELIABLE_RELAY
        | message_type::UNRELIABLE_RELAY
        | message_type::LINGER_DATA_FRAME
        | message_type::USER_MESSAGE => ClientMessage::Ignored,
        other => ClientMessage::Unknown(other),
    })
}

/// CryptoAPI blob constants (Microsoft Enhanced Cryptographic Provider v1.0).
pub mod blob {
    pub const PUBLICKEYBLOB: u8 = 6;
    pub const SIMPLEBLOB: u8 = 1;
    pub const CUR_BLOB_VERSION: u8 = 2;
    pub const CALG_RSA_KEYX: u32 = 0xa400;
    pub const CALG_RC4: u32 = 0x6801;
    /// "RSA1".
    pub const RSA1: u32 = 0x3141_5352;
}

/// Modulus (little-endian, as stored) and public exponent of a PUBLICKEYBLOB.
pub fn parse_public_key_blob(b: &[u8]) -> Result<(&[u8], u32), WireError> {
    let mut r = Reader::new(b);
    let kind = r.u8()?;
    let version = r.u8()?;
    let _reserved = r.u16()?;
    let _alg = r.i32()?;
    let magic = r.i32()? as u32;
    let bits = r.i32()?;
    let exponent = r.i32()? as u32;
    if kind != blob::PUBLICKEYBLOB
        || version != blob::CUR_BLOB_VERSION
        || magic != blob::RSA1
        || !(512..=4096).contains(&bits)
        || bits % 8 != 0
    {
        return Err(WireError::Length);
    }
    let modulus = r.take((bits / 8) as usize)?;
    Ok((modulus, exponent))
}

/// SIMPLEBLOB wrapping an RC4 key: header, the exchange algorithm and the RSA
/// ciphertext in CryptoAPI's little-endian byte order.
pub fn simple_blob(ciphertext_be: &[u8]) -> Vec<u8> {
    let mut out = vec![blob::SIMPLEBLOB, blob::CUR_BLOB_VERSION, 0, 0];
    out.extend_from_slice(&blob::CALG_RC4.to_le_bytes());
    out.extend_from_slice(&blob::CALG_RSA_KEYX.to_le_bytes());
    out.extend(ciphertext_be.iter().rev());
    out
}
