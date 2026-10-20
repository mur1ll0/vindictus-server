//! Loopback, unencrypted HeroesChannelServer for the explicit minimum profile.
use super::lab_frontend::RuntimeError;
use crate::{
    domain::map_partition::{Channel, OutgoingPacket, Topology},
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        minimum_map::{self as wire, Kind, Message},
        transport::PlainReceiver,
    },
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, mpsc, watch},
    task::JoinSet,
};

struct Ticket {
    key: i32,
    partition: i64,
    action: wire::ActionSync,
    look: OutgoingPacket,
}
struct Peer {
    token: uuid::Uuid,
    packets: mpsc::Sender<OutgoingPacket>,
    stop: watch::Sender<bool>,
}
struct State {
    channel: Channel,
    pending: BTreeMap<i64, Ticket>,
    peers: BTreeMap<i64, Peer>,
}
pub struct Runtime {
    state: Mutex<State>,
}
impl Runtime {
    pub fn new(topology: Topology) -> Self {
        Self {
            state: Mutex::new(State {
                channel: Channel::new(topology),
                pending: BTreeMap::new(),
                peers: BTreeMap::new(),
            }),
        }
    }
    pub async fn enter(
        &self,
        cid: i64,
        requested: i64,
        partition: i64,
        action: wire::ActionSync,
        look: OutgoingPacket,
    ) -> Result<i32, RuntimeError> {
        // One laboratory channel; -1/0 request recommendation. Explicit unavailable channels fail.
        if requested > 1 {
            return Err(RuntimeError::Message);
        }
        let key = i32::from_le_bytes(
            uuid::Uuid::new_v4().as_bytes()[..4]
                .try_into()
                .map_err(|_| RuntimeError::Operation)?,
        ) & i32::MAX;
        let mut state = self.state.lock().await;
        if state.peers.contains_key(&cid) {
            return Err(RuntimeError::Message);
        }
        state.pending.insert(
            cid,
            Ticket {
                key,
                partition,
                action,
                look,
            },
        );
        Ok(key)
    }
    pub async fn remove(&self, cid: i64, pool: &sqlx::PgPool) -> Result<(), RuntimeError> {
        self.remove_connection(cid, None, pool).await
    }
    async fn remove_connection(
        &self,
        cid: i64,
        token: Option<uuid::Uuid>,
        pool: &sqlx::PgPool,
    ) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().await;
        if token.is_some_and(|t| state.peers.get(&cid).is_none_or(|p| p.token != t)) {
            return Ok(());
        }
        state.pending.remove(&cid);
        if let Some(peer) = state.peers.remove(&cid) {
            let _ = peer.stop.send(true);
        }
        let packets = state.channel.leave(cid);
        state.broadcast(packets);
        sqlx::query("UPDATE game_character_info SET channel=NULL WHERE id=$1")
            .bind(cid)
            .execute(pool)
            .await
            .map_err(|_| RuntimeError::Operation)?;
        Ok(())
    }
    pub async fn member_count(&self) -> usize {
        self.state.lock().await.channel.member_count()
    }
}
impl State {
    fn broadcast(&mut self, packets: BTreeMap<i64, Vec<OutgoingPacket>>) {
        for (cid, packets) in packets {
            if let Some(peer) = self.peers.get(&cid) {
                for packet in packets {
                    if peer.packets.try_send(packet).is_err() {
                        let _ = peer.stop.send(true);
                        break;
                    }
                }
            }
        }
    }
}
async fn write(stream: &mut TcpStream, packet: OutgoingPacket) -> Result<(), RuntimeError> {
    let bytes = Frame {
        instance_id: 0,
        category_id: packet.category,
        body: &packet.body,
    }
    .encode(DEFAULT_MAX_BODY)
    .map_err(|_| RuntimeError::Message)?;
    tokio::time::timeout(Duration::from_secs(5), stream.write_all(&bytes))
        .await
        .map_err(|_| RuntimeError::Timeout)??;
    Ok(())
}
async fn failed(stream: &mut TcpStream, reason: &str) -> Result<(), RuntimeError> {
    let body = wire::encode(
        Kind::IdentifyFailed,
        Some(&Message::IdentifyFailed(Some(reason))),
        DEFAULT_MAX_BODY,
    )
    .map_err(|_| RuntimeError::Message)?;
    write(
        stream,
        OutgoingPacket {
            category: Kind::IdentifyFailed.category(),
            body: body.to_vec(),
        },
    )
    .await
}
pub async fn connection(
    mut stream: TcpStream,
    runtime: Arc<Runtime>,
    pool: sqlx::PgPool,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), RuntimeError> {
    if !stream.peer_addr()?.ip().is_loopback() {
        return Err(RuntimeError::Profile);
    }
    let mut cid = None;
    let token = uuid::Uuid::new_v4();
    let mut receiver = PlainReceiver::new(DEFAULT_MAX_BODY);
    let (packets, mut inbox) = mpsc::channel(256);
    let (stop, mut stopped) = watch::channel(false);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let result = async {
        let mut buffer = [0;4096];
        loop {
            let count = tokio::select! {
                _ = shutdown.changed() => return Ok(()),
                _ = stopped.changed() => return Ok(()),
                _ = tokio::time::sleep_until(deadline), if cid.is_none() => return Err(RuntimeError::Timeout),
                packet = inbox.recv() => { if let Some(packet) = packet { write(&mut stream, packet).await?; } continue; }
                count = stream.read(&mut buffer) => count?,
            };
            if count == 0 { return if receiver.buffered_len() == 0 { Ok(()) } else { Err(RuntimeError::Message) }; }
            for frame in receiver.push(&buffer[..count]).map_err(|_| RuntimeError::Message)? {
                if cid.is_none() {
                    let identify = wire::decode_ingress(Kind::Identify, frame.category_id, &frame.body, DEFAULT_MAX_BODY);
                    let Ok(Some(Message::Identify { id, key })) = identify else {
                        failed(&mut stream, "GameUI_Heroes_Channel_Error_IdentifyFail").await?; return Ok(());
                    };
                    let mut state = runtime.state.lock().await;
                    // Remove before comparing: an incorrect key burns the outstanding ticket too.
                    let ticket = state.pending.remove(&id).filter(|t| t.key == key);
                    let Some(ticket) = ticket else {
                        drop(state);
                        failed(&mut stream, "GameUI_Heroes_Channel_Error_KeyMismatch").await?; return Ok(());
                    };
                    let changed = sqlx::query("UPDATE game_character_info SET channel=1 WHERE id=$1 AND is_connected").bind(id).execute(&pool).await.map_err(|_| RuntimeError::Operation)?.rows_affected();
                    if changed != 1 { return Err(RuntimeError::Operation); }
                    cid = Some(id);
                    state.peers.insert(id, Peer { token, packets: packets.clone(), stop: stop.clone() });
                    let output = state.channel.enter_with_look_packet(id, ticket.partition, ticket.look, ticket.action).map_err(|_| RuntimeError::Message)?;
                    state.broadcast(output);
                    tracing::info!("minimum channel identified and entered; values omitted");
                } else {
                    let id = cid.ok_or(RuntimeError::Message)?;
                    let mut state = runtime.state.lock().await;
                    if state.peers.get(&id).is_none_or(|p| p.token != token) { return Ok(()); }
                    let kind = [Kind::MovePartition, Kind::UpdateAction].into_iter().find(|k|
                        frame.category_id == k.category() || (frame.category_id == 0 && frame.body.get(..16) == Some(k.type_id().as_bytes().as_slice()))
                    ).ok_or(RuntimeError::Message)?;
                    let output = match wire::decode_ingress(kind, frame.category_id, &frame.body, DEFAULT_MAX_BODY).map_err(|_| RuntimeError::Message)? {
                        Some(Message::MovePartition { target_partition_id }) => state.channel.move_partition(id, target_partition_id),
                        Some(Message::UpdateAction { data }) => state.channel.update_action(id, data),
                        _ => return Err(RuntimeError::Message),
                    }.map_err(|_| RuntimeError::Message)?;
                    state.broadcast(output);
                }
            }
        }
    }.await;
    if let Some(id) = cid {
        runtime.remove_connection(id, Some(token), &pool).await?;
    }
    let _ = stream.shutdown().await;
    result
}
pub async fn run(
    listener: TcpListener,
    runtime: Arc<Runtime>,
    pool: sqlx::PgPool,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), RuntimeError> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(RuntimeError::Profile);
    }
    let (stop, signal) = watch::channel(false);
    let mut tasks = JoinSet::new();
    tokio::pin!(shutdown);
    let result = loop {
        tokio::select! {
            () = &mut shutdown => break Ok(()),
            Some(_) = tasks.join_next(), if !tasks.is_empty() => {},
            accepted = listener.accept(), if tasks.len() < super::lab_frontend::MAX_CONNECTIONS => {
                match accepted {
                    Ok((stream,_)) => {
                        let (r,p,s) = (runtime.clone(), pool.clone(), signal.clone());
                        tasks.spawn(async move { if connection(stream,r,p,s).await.is_err() { tracing::warn!("minimum channel connection ended; values omitted"); } });
                    }
                    Err(e) => break Err(RuntimeError::Io(e)),
                }
            }
        }
    };
    let _ = stop.send(true);
    while tasks.join_next().await.is_some() {}
    result
}
