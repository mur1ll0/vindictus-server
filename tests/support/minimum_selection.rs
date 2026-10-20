use super::*;
use tokio::net::TcpStream;
use vindictus_server::{
    domain::map_partition::{MapFile, Topology},
    protocol::{
        minimum_map::{self as wire, Kind, Message},
        transport::{OwnedFrame, PlainReceiver},
    },
};

pub(super) fn topology() -> Topology {
    let mut bytes = vec![0; 24];
    bytes.extend(2_i32.to_le_bytes());
    bytes.extend(1_i64.to_le_bytes());
    bytes.extend(2_i64.to_le_bytes());
    bytes.extend(1_i64.to_le_bytes());
    bytes.extend(2_i64.to_le_bytes());
    Topology::from_maps(&[MapFile::parse("synthetic", &bytes).unwrap()])
}
async fn plain_send(stream: &mut TcpStream, message: Message<'_>) {
    let body = wire::encode(message.kind(), Some(&message), DEFAULT_MAX_BODY).unwrap();
    let bytes = Frame {
        instance_id: 0,
        category_id: message.kind().category(),
        body: &body,
    }
    .encode(DEFAULT_MAX_BODY)
    .unwrap();
    // Deliberate TCP fragmentation, including the framing header.
    for chunk in bytes.chunks(3) {
        stream.write_all(chunk).await.unwrap();
    }
}
async fn plain_read(stream: &mut TcpStream, receiver: &mut PlainReceiver) -> OwnedFrame {
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut byte = [0];
        loop {
            stream.read_exact(&mut byte).await.unwrap();
            if let Some(frame) = receiver.push(&byte).unwrap().pop() {
                return frame;
            }
        }
    })
    .await
    .unwrap()
}
async fn frontend_send(
    client: &mut TcpStream,
    tx: &mut vindictus_server::protocol::transport::LegacySender,
    message: Message<'_>,
) {
    let body = wire::encode(message.kind(), Some(&message), DEFAULT_MAX_BODY).unwrap();
    send(client, tx, message.kind().category(), &body).await;
}
pub(super) async fn locked_selection(
    client: &mut TcpStream,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
    tx: &mut vindictus_server::protocol::transport::LegacySender,
) {
    frontend_send(client, tx, Message::SelectCharacter { index: 0 }).await;
    assert_eq!(read(client, rx).await.category_id, CKind::System.category());
}
pub(super) async fn exercise(
    r: &std::sync::Arc<Runtime>,
    pool: &sqlx::PgPool,
    cid: i64,
    client: &mut TcpStream,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
    tx: &mut vindictus_server::protocol::transport::LegacySender,
) {
    // CharacterSN is not the list index (only list index zero is admitted).
    let sn: i32 = sqlx::query_scalar("SELECT character_sn FROM game_character_info WHERE id=$1")
        .bind(cid)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_ne!(sn, 0);
    sqlx::query("INSERT INTO game_micro_play_status VALUES($1,0,0,0,0,0,0,0,0,0,decode('5100','hex'),NULL) ON CONFLICT(cid) DO UPDATE SET current_quest_utf16=EXCLUDED.current_quest_utf16").bind(cid).execute(pool).await.unwrap();
    frontend_send(client, tx, Message::SelectCharacter { index: 0 }).await;
    for kind in [
        Kind::LoginOk,
        Kind::NoticeGameEnvironment,
        Kind::CharacterCommonInfo,
        Kind::UpdateStat,
    ] {
        let frame = read(client, rx).await;
        assert_eq!(frame.category_id, kind.category());
        let message = wire::decode(kind, &frame.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap();
        match message {
            Message::LoginOk {
                user_care_type,
                user_care_next_state,
                ..
            } => {
                // UserInfo defaults and UserCareType.Nothing: zero activates the event.
                assert_eq!(user_care_type, -1);
                assert_eq!(user_care_next_state, -1);
            }
            Message::CharacterCommonInfo {
                query_id,
                info: Some(info),
            } => {
                assert_eq!(query_id, 0);
                assert_eq!(info.cid, cid);
                assert!(info.costume.is_some());
            }
            Message::UpdateStat { stat: Some(stat) } => {
                assert_eq!(stat.len(), 25);
                assert!(stat.iter().any(|(k, v)| *k == "DEF" && *v == 9));
                assert!(stat.iter().any(|(k, v)| *k == "HP" && *v > 0));
            }
            Message::NoticeGameEnvironment { cafe_type, is_otp } => {
                assert_eq!(cafe_type, 0);
                assert!(!is_otp);
            }
            _ => {}
        }
    }
    let state:(bool,Option<i64>,Option<Vec<u8>>)=sqlx::query_as("SELECT is_connected,(SELECT last_login_cid FROM game_user_info WHERE nexon_sn=uid),(SELECT current_quest_utf16 FROM game_micro_play_status WHERE cid=id) FROM game_character_info WHERE id=$1").bind(cid).fetch_one(pool).await.unwrap();
    assert_eq!(state, (true, Some(cid), None));
    let (bind, channel) = r.channel().unwrap();
    let listener = tokio::net::TcpListener::bind(bind).await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, mut signal) = tokio::sync::watch::channel(false);
    let p = pool.clone();
    let channel_task = tokio::spawn(vindictus_server::infrastructure::channel_server::run(
        listener,
        channel.clone(),
        p,
        async move {
            let _ = signal.changed().await;
        },
    ));
    let enter = || Message::EnterChannel {
        channel_id: -1,
        partition_id: 1,
        action: Default::default(),
    };
    frontend_send(client, tx, enter()).await;
    let frame = read(client, rx).await;
    let Some(Message::ChannelServerAddress {
        key, channel_id, ..
    }) = wire::decode(Kind::ChannelServerAddress, &frame.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("channel address missing")
    };
    assert_eq!(channel_id, 1);
    let mut invalid = TcpStream::connect(address).await.unwrap();
    plain_send(
        &mut invalid,
        Message::UpdateAction {
            data: Default::default(),
        },
    )
    .await;
    assert_eq!(
        plain_read(&mut invalid, &mut PlainReceiver::new(DEFAULT_MAX_BODY))
            .await
            .category_id,
        366
    );
    // Wrong key burns the ticket; even the correct key cannot be replayed afterwards.
    for key in [key.wrapping_add(1), key] {
        let mut socket = TcpStream::connect(address).await.unwrap();
        plain_send(&mut socket, Message::Identify { id: cid, key }).await;
        let frame = plain_read(&mut socket, &mut PlainReceiver::new(DEFAULT_MAX_BODY)).await;
        assert_eq!(frame.category_id, 366);
    }
    frontend_send(client, tx, enter()).await;
    let frame = read(client, rx).await;
    let Some(Message::ChannelServerAddress { key, .. }) =
        wire::decode(Kind::ChannelServerAddress, &frame.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("channel address missing")
    };
    let mut socket = TcpStream::connect(address).await.unwrap();
    let mut plain = PlainReceiver::new(DEFAULT_MAX_BODY);
    plain_send(&mut socket, Message::Identify { id: cid, key }).await;
    assert_eq!(plain_read(&mut socket, &mut plain).await.category_id, 373);
    assert_eq!(plain_read(&mut socket, &mut plain).await.category_id, 374);
    let action = wire::ActionSync {
        sequence: 19,
        position: wire::Vector3D {
            x_bits: 7.5_f32.to_bits(),
            ..Default::default()
        },
        ..Default::default()
    };
    plain_send(
        &mut socket,
        Message::MovePartition {
            target_partition_id: 2,
        },
    )
    .await;
    plain_send(&mut socket, Message::UpdateAction { data: action }).await;
    loop {
        let frame = plain_read(&mut socket, &mut plain).await;
        if frame.category_id != Kind::NotifyAction.category() {
            continue;
        }
        if let Some(Message::NotifyAction { id, action: actual }) = wire::decode_ingress(
            Kind::NotifyAction,
            frame.category_id,
            &frame.body,
            DEFAULT_MAX_BODY,
        )
        .unwrap()
            && actual == action
        {
            assert_eq!(id, cid);
            break;
        }
    }
    assert_eq!(channel.member_count().await, 1);
    // Closing only the channel removes membership while leaving frontend selection alive.
    socket.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        while channel.member_count().await != 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    frontend_send(client, tx, enter()).await;
    let frame = read(client, rx).await;
    let Some(Message::ChannelServerAddress { key, .. }) =
        wire::decode(Kind::ChannelServerAddress, &frame.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("channel address missing")
    };
    client.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let state: (bool, Option<i16>) =
                sqlx::query_as("SELECT is_connected,channel FROM game_character_info WHERE id=$1")
                    .bind(cid)
                    .fetch_one(pool)
                    .await
                    .unwrap();
            if state == (false, None) && channel.member_count().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // Frontend logout revokes an Identify ticket that has not been consumed.
    let mut replay = TcpStream::connect(address).await.unwrap();
    plain_send(&mut replay, Message::Identify { id: cid, key }).await;
    assert_eq!(
        plain_read(&mut replay, &mut PlainReceiver::new(DEFAULT_MAX_BODY))
            .await
            .category_id,
        366
    );
    let _ = stop.send(true);
    channel_task.await.unwrap().unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_minimum_selection_channel_identify_movement_and_disconnect() {
    super::minimum_flow(true).await;
}
