//! Minimal ProudNet relay for mission P2P groups (RelayServer over Nettention.Proud):
//! AddClientWaiting keys, the connection handshake, NMInfosResult, P2P group joins with
//! their acknowledgments and ReliablePong. Relay frames have no receiver while a group
//! has one member. Events reach the MicroPlay through `RelayEvent`.
use crate::protocol::proudnet::{self as wire, ClientMessage, Settings};
use rand_core::{OsRng, RngCore};
use rsa::{BigUint, Pkcs1v15Encrypt, RsaPublicKey};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
};
use zeroize::Zeroizing;

/// RelayServer timeouts: waiting for the TCP connection and for NMInfosResult.
const WAIT_CONNECTION: Duration = Duration::from_secs(60);
const WAIT_NM_INFO: Duration = Duration::from_secs(30);
/// Bounded per-connection input buffer and outbound queue.
const MAX_BUFFER: usize = 4 * wire::MAX_MESSAGE;
const OUTBOUND: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RelayEvent {
    /// NMInfosResult accepted: the relay client is Connected.
    Connected {
        cid: i64,
        host_id: i32,
    },
    /// P2PGroupJoinMemberAckComplete for the member.
    Joined {
        group: i32,
        cid: i64,
    },
    Closed {
        cid: i64,
    },
}

/// RelayClient registration returned to AddPlayer (ConnectionRequestMessage values).
#[derive(Clone, Copy, Debug)]
pub struct Waiting {
    pub sn: i32,
    pub key: i32,
}

struct Pending {
    key: i32,
    cid: i64,
    nexon_sn: i32,
}
struct Client {
    cid: i64,
    nexon_sn: i32,
    connected: bool,
    out: mpsc::Sender<Vec<u8>>,
}
struct Group {
    members: Vec<i32>,
}
#[derive(Default)]
struct State {
    next_sn: i32,
    next_host: i32,
    next_event: i32,
    next_nm_id: i64,
    waiting: HashMap<i32, Pending>,
    /// Accepted (after the connection request) by CID → HostID.
    accepted: HashMap<i64, i32>,
    clients: HashMap<i32, Client>,
    groups: HashMap<i32, Group>,
}
impl State {
    fn issue_host(&mut self) -> i32 {
        // HostID 0 is none and 1 is the server.
        self.next_host = self.next_host.max(1) + 1;
        self.next_host
    }
}

#[derive(Clone)]
pub struct Relay {
    state: Arc<Mutex<State>>,
    events: broadcast::Sender<RelayEvent>,
    public: SocketAddr,
    instance: [u8; 16],
}

fn lock(m: &Mutex<State>) -> std::sync::MutexGuard<'_, State> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Relay {
    /// `public` is the endpoint sent to clients in ConnectionRequestMessage.
    pub fn new(public: SocketAddr) -> Self {
        let mut instance = [0; 16];
        OsRng.fill_bytes(&mut instance);
        let (events, _) = broadcast::channel(256);
        Self {
            state: Arc::new(Mutex::new(State {
                next_nm_id: 1,
                ..State::default()
            })),
            events,
            public,
            instance,
        }
    }
    pub fn endpoint(&self) -> SocketAddr {
        self.public
    }
    pub fn subscribe(&self) -> broadcast::Receiver<RelayEvent> {
        self.events.subscribe()
    }
    /// RelayServer.AddClientWaiting: a fresh serial number and a random key.
    pub fn add_client_waiting(&self, cid: i64, nexon_sn: i32) -> Waiting {
        let mut s = lock(&self.state);
        let mut sn = s.next_sn;
        while s.waiting.contains_key(&sn) {
            sn = if sn == i32::MAX { 0 } else { sn + 1 };
        }
        s.next_sn = if sn == i32::MAX { 0 } else { sn + 1 };
        // System.Random.Next(): [0, int.MaxValue).
        let key = (OsRng.next_u32() & 0x7fff_ffff) as i32 % i32::MAX;
        s.waiting.insert(sn, Pending { key, cid, nexon_sn });
        let state = self.state.clone();
        tokio::spawn(async move {
            tokio::time::sleep(WAIT_CONNECTION).await;
            lock(&state).waiting.remove(&sn);
        });
        Waiting { sn, key }
    }
    /// RelayServer.MakeP2PGroup: CreateP2PGroup without members. Its custom field
    /// (ParentCategory, ParentID, 0) only reaches initial members, and there are none.
    pub fn make_group(&self) -> i32 {
        let mut s = lock(&self.state);
        let id = s.issue_host();
        s.groups.insert(
            id,
            Group {
                members: Vec::new(),
            },
        );
        id
    }
    /// P2PGroup.Join: JoinP2PGroup(member, group, custom field) and SendNMInfos of the
    /// members already in the group to the joining client. The custom field is ParentID,
    /// ParentCategory, ParentID, 1 and the joining member's HostID, CID and NexonSN; the
    /// client's P2PMemberJoin handler (engine.dll 0x42351740) reads it unbounded, so a
    /// shorter field makes it read past the buffer. False when the client is not connected.
    pub fn join(&self, group: i32, cid: i64, parent_id: i64, category: &str) -> bool {
        let mut s = lock(&self.state);
        let Some((&host, client)) = s.clients.iter().find(|(_, c)| c.cid == cid && c.connected)
        else {
            return false;
        };
        let out = client.out.clone();
        let nexon_sn = client.nexon_sn;
        let Some(g) = s.groups.get(&group) else {
            return false;
        };
        if g.members.contains(&host) {
            return false;
        }
        let previous: Vec<i32> = g.members.clone();
        let mut members = previous.clone();
        members.push(host);
        let mut join_field = Vec::new();
        join_field.extend_from_slice(&parent_id.to_le_bytes());
        write_dotnet_string(&mut join_field, category);
        join_field.extend_from_slice(&parent_id.to_le_bytes());
        join_field.extend_from_slice(&1_i32.to_le_bytes());
        join_field.extend_from_slice(&host.to_le_bytes());
        join_field.extend_from_slice(&cid.to_le_bytes());
        join_field.extend_from_slice(&nexon_sn.to_le_bytes());
        s.next_event += 1;
        let event = s.next_event;
        let nm_id = s.next_nm_id;
        s.next_nm_id += 1;
        let infos: Vec<(i32, i64, i32)> = previous
            .iter()
            .filter_map(|h| s.clients.get(h).map(|c| (*h, c.cid, c.nexon_sn)))
            .collect();
        if let Some(g) = s.groups.get_mut(&group) {
            g.members = members.clone();
        }
        drop(s);
        // The new member learns about every member, itself included.
        for m in &members {
            let msg =
                wire::member_join_unencrypted(group, *m, &join_field, event, 0, &[0; 16], false, 0);
            if out.try_send(msg).is_err() {
                return false;
            }
        }
        out.try_send(wire::nm_infos(nm_id, &infos)).is_ok()
    }
    /// P2PGroup.Leave / Close.
    pub fn close_group(&self, group: i32) {
        lock(&self.state).groups.remove(&group);
    }

    /// Accepts relay connections until the listener fails.
    pub async fn serve(self, listener: TcpListener) {
        loop {
            let Ok((stream, peer)) = listener.accept().await else {
                continue;
            };
            let relay = self.clone();
            tokio::spawn(async move {
                tracing::info!("relay connection opened");
                match relay.connection(stream, peer).await {
                    Ok(()) => tracing::info!("relay connection closed"),
                    Err(e) => tracing::info!(error = %e, "relay connection closed by an I/O error"),
                }
            });
        }
    }

    async fn connection(&self, stream: TcpStream, peer: SocketAddr) -> std::io::Result<()> {
        let (mut rd, mut wr) = stream.into_split();
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(OUTBOUND);
        let writer = tokio::spawn(async move {
            while let Some(payload) = rx.recv().await {
                let mut framed = Vec::with_capacity(payload.len() + 8);
                wire::frame(&payload, &mut framed);
                if wr.write_all(&framed).await.is_err() {
                    break;
                }
            }
        });
        let _ = tx
            .send(wire::connection_hint(false, &Settings::relay()))
            .await;
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0_u8; 8192];
        let mut host: Option<i32> = None;
        let mut session_key: Option<Zeroizing<[u8; 16]>> = None;
        let mut noted = 0_u32;
        let result = loop {
            let read = tokio::time::timeout(WAIT_CONNECTION, rd.read(&mut chunk)).await;
            let n = match read {
                Ok(Ok(0)) | Err(_) => break Ok(()),
                Ok(Ok(n)) => n,
                Ok(Err(e)) => break Err(e),
            };
            if buf.len() + n > MAX_BUFFER {
                break Ok(());
            }
            buf.extend_from_slice(&chunk[..n]);
            let mut stop = false;
            loop {
                match wire::extract(&buf) {
                    Ok(Some((payload, used))) => {
                        buf.drain(..used);
                        if !self
                            .handle(&payload, &tx, peer, &mut host, &mut session_key, &mut noted)
                            .await
                        {
                            stop = true;
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => {
                        stop = true;
                        break;
                    }
                }
            }
            if stop {
                break Ok(());
            }
        };
        if let Some(h) = host {
            let removed = {
                let mut s = lock(&self.state);
                for g in s.groups.values_mut() {
                    g.members.retain(|m| *m != h);
                }
                s.clients.remove(&h)
            };
            if let Some(c) = removed {
                s_accepted_remove(&self.state, c.cid);
                let _ = self.events.send(RelayEvent::Closed { cid: c.cid });
            }
        }
        drop(tx);
        let _ = writer.await;
        result
    }

    /// One client message; false closes the connection.
    async fn handle(
        &self,
        payload: &[u8],
        tx: &mpsc::Sender<Vec<u8>>,
        peer: SocketAddr,
        host: &mut Option<i32>,
        session_key: &mut Option<Zeroizing<[u8; 16]>>,
        noted: &mut u32,
    ) -> bool {
        let Ok(message) = wire::parse_client(payload) else {
            tracing::info!(
                length = payload.len(),
                first = payload.first().copied(),
                "relay message unparsed; closing"
            );
            return false;
        };
        let decrypted: Vec<u8>;
        let message = match message {
            ClientMessage::Encrypted {
                reliable,
                mode,
                data,
            } => {
                let Some(key) = session_key.as_ref() else {
                    tracing::info!("relay encrypted message before the session key; closing");
                    return false;
                };
                let Ok((count, inner)) = wire::decrypt_message(&key[..], reliable, data) else {
                    tracing::info!("relay encrypted message too short; closing");
                    return false;
                };
                decrypted = inner;
                let rmi = (decrypted.first() == Some(&wire::message_type::RMI)
                    && decrypted.len() >= 3)
                    .then(|| u16::from_le_bytes([decrypted[1], decrypted[2]]));
                *noted += 1;
                if *noted <= 16 {
                    tracing::info!(
                        reliable,
                        mode,
                        count,
                        inner = decrypted.first().copied(),
                        rmi,
                        "relay encrypted message decrypted"
                    );
                }
                match wire::parse_client(&decrypted) {
                    Ok(
                        ClientMessage::Encrypted { .. }
                        | ClientMessage::PublicKey(_)
                        | ClientMessage::RequestConnection { .. },
                    )
                    | Err(_) => {
                        tracing::info!("relay encrypted inner message invalid; closing");
                        return false;
                    }
                    Ok(inner) => inner,
                }
            }
            other => other,
        };
        match message {
            ClientMessage::PublicKey(blob) => {
                let Some((encrypted, key)) = encrypt_session_key(blob) else {
                    tracing::info!("relay public key rejected; closing");
                    return false;
                };
                *session_key = Some(key);
                tracing::info!("relay session key sent");
                tx.send(wire::session_key_success(&encrypted)).await.is_ok()
            }
            ClientMessage::RequestConnection {
                user_data,
                protocol,
                internal_version,
            } => {
                // Protocol constants only (no key material): evidence for the client build.
                tracing::info!(
                    protocol_guid = %wire::guid_string(&protocol),
                    internal_version,
                    "relay connection request received"
                );
                let admitted = (protocol == wire::PROTOCOL_VERSION && user_data.len() == 8)
                    .then(|| {
                        let sn = i32::from_le_bytes([
                            user_data[0],
                            user_data[1],
                            user_data[2],
                            user_data[3],
                        ]);
                        let key = i32::from_le_bytes([
                            user_data[4],
                            user_data[5],
                            user_data[6],
                            user_data[7],
                        ]);
                        let mut s = lock(&self.state);
                        if s.waiting.get(&sn)?.key != key {
                            return None;
                        }
                        let pending = s.waiting.remove(&sn)?;
                        let h = s.issue_host();
                        s.accepted.insert(pending.cid, h);
                        s.clients.insert(
                            h,
                            Client {
                                cid: pending.cid,
                                nexon_sn: pending.nexon_sn,
                                connected: false,
                                out: tx.clone(),
                            },
                        );
                        Some((h, pending.cid, pending.nexon_sn))
                    })
                    .flatten();
                let Some((h, cid, nexon_sn)) = admitted else {
                    tracing::info!(
                        protocol_matches = protocol == wire::PROTOCOL_VERSION,
                        user_data_length = user_data.len(),
                        "relay connection request denied"
                    );
                    let _ = tx.send(wire::denied()).await;
                    return false;
                };
                tracing::info!("relay connection request admitted");
                *host = Some(h);
                let state = self.state.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(WAIT_NM_INFO).await;
                    let s = lock(&state);
                    if s.clients.get(&h).is_some_and(|c| !c.connected) {
                        // The connection task removes the client when it closes.
                        if let Some(c) = s.clients.get(&h) {
                            let _ = c.out.try_send(wire::denied());
                        }
                    }
                });
                let mut reply = Vec::with_capacity(12);
                reply.extend_from_slice(&cid.to_le_bytes());
                reply.extend_from_slice(&nexon_sn.to_le_bytes());
                let ip = match peer.ip() {
                    std::net::IpAddr::V4(v) => v.octets(),
                    std::net::IpAddr::V6(_) => [0; 4],
                };
                tx.send(wire::connect_success(
                    h,
                    &self.instance,
                    &reply,
                    ip,
                    peer.port(),
                ))
                .await
                .is_ok()
            }
            ClientMessage::NmInfosResult(cid) => {
                let Some(h) = *host else { return false };
                let ok = {
                    let mut s = lock(&self.state);
                    let accepted = s.accepted.remove(&cid) == Some(h);
                    match s.clients.get_mut(&h) {
                        Some(c) if accepted && c.cid == cid => {
                            c.connected = true;
                            true
                        }
                        _ => false,
                    }
                };
                tracing::info!(accepted = ok, "relay NM infos result");
                if ok {
                    let _ = self.events.send(RelayEvent::Connected { cid, host_id: h });
                }
                ok
            }
            ClientMessage::MemberJoinAck { group, member, .. } => {
                let cid = {
                    let s = lock(&self.state);
                    (Some(member) == *host)
                        .then(|| s.clients.get(&member).map(|c| c.cid))
                        .flatten()
                };
                if let Some(cid) = cid {
                    let _ = self.events.send(RelayEvent::Joined { group, cid });
                }
                true
            }
            ClientMessage::ReliablePing => tx
                .send(wire::Writer::rmi(wire::rmi::RELIABLE_PONG).finish())
                .await
                .is_ok(),
            ClientMessage::ShutdownTcpHandshake => false,
            ClientMessage::Ignored => {
                let rmi = (payload.first() == Some(&wire::message_type::RMI) && payload.len() >= 3)
                    .then(|| u16::from_le_bytes([payload[1], payload[2]]));
                note(
                    noted,
                    "relay message ignored",
                    payload.first().copied(),
                    rmi,
                );
                true
            }
            ClientMessage::Unknown(kind) => {
                note(
                    noted,
                    "relay message type unknown; ignored",
                    Some(kind),
                    None,
                );
                true
            }
            ClientMessage::Encrypted { .. } => false,
        }
    }
}

/// Diagnostics for the first messages of a connection that the relay does not act on.
fn note(noted: &mut u32, what: &'static str, kind: Option<u8>, rmi: Option<u16>) {
    *noted += 1;
    if *noted <= 16 {
        tracing::info!(kind, rmi, "{what}");
    }
}

fn s_accepted_remove(state: &Mutex<State>, cid: i64) {
    lock(state).accepted.remove(&cid);
}

/// .NET BinaryWriter.Write(string): 7-bit encoded UTF-8 length, then the bytes.
fn write_dotnet_string(out: &mut Vec<u8>, s: &str) {
    let mut n = s.len();
    loop {
        let b = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
    out.extend_from_slice(s.as_bytes());
}

/// CCrypto: a 128-bit RC4 session key encrypted with the client's exchange public key
/// (PKCS#1 v1.5), as a SIMPLEBLOB the client imports with its private key.
/// Returns the SIMPLEBLOB and the session key the relay keeps to decrypt types 0x24/0x25.
pub fn encrypt_session_key(public_key_blob: &[u8]) -> Option<(Vec<u8>, Zeroizing<[u8; 16]>)> {
    let (modulus_le, exponent) = wire::parse_public_key_blob(public_key_blob).ok()?;
    let key =
        RsaPublicKey::new(BigUint::from_bytes_le(modulus_le), BigUint::from(exponent)).ok()?;
    let mut session = Zeroizing::new([0_u8; 16]);
    OsRng.fill_bytes(&mut session[..]);
    let ciphertext = key
        .encrypt(&mut OsRng, Pkcs1v15Encrypt, &session[..])
        .ok()?;
    Some((wire::simple_blob(&ciphertext), session))
}
