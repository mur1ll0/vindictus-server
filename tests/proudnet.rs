use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, traits::PublicKeyParts};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use vindictus_server::{
    infrastructure::relay::{Relay, RelayEvent, encrypt_session_key},
    protocol::proudnet::{self as wire, ClientMessage, Reader, Writer, message_type, rmi},
};

/// CryptoAPI PUBLICKEYBLOB of an RSA key (AT_KEYEXCHANGE).
fn public_key_blob(key: &RsaPrivateKey) -> Vec<u8> {
    let bits = key.n().bits() as u32;
    let mut b = vec![6, 2, 0, 0];
    b.extend_from_slice(&0xa400_u32.to_le_bytes());
    b.extend_from_slice(&0x3141_5352_u32.to_le_bytes());
    b.extend_from_slice(&bits.to_le_bytes());
    let e = key.e().to_bytes_le();
    let mut exp = [0_u8; 4];
    exp[..e.len()].copy_from_slice(&e);
    b.extend_from_slice(&exp);
    let mut n = key.n().to_bytes_le();
    n.resize((bits / 8) as usize, 0);
    b.extend_from_slice(&n);
    b
}

#[test]
fn framing_compact_widths_and_messages_follow_the_recovered_layout() {
    for (v, width) in [
        (0, 1),
        (-128, 1),
        (127, 1),
        (128, 2),
        (-32768, 2),
        (40000, 4),
        (1 << 40, 8),
    ] {
        let mut out = Vec::new();
        wire::put_compact(&mut out, v);
        assert_eq!((out[0], out.len()), (width, 1 + width as usize));
        assert_eq!(Reader::new(&out).compact().unwrap(), v);
    }
    let mut framed = Vec::new();
    wire::frame(&[4, 1, 2], &mut framed);
    assert_eq!(framed, [0x13, 0x57, 1, 3, 4, 1, 2]);
    assert_eq!(wire::extract(&framed[..5]).unwrap(), None);
    assert_eq!(
        wire::extract(&framed).unwrap(),
        Some((vec![4, 1, 2], framed.len()))
    );
    assert_eq!(wire::extract(&[0, 0, 1, 0]), Err(wire::WireError::Splitter));
    // Hint: type, flag, then 1+4+8+1+4+1+4+4×1+4+1 bytes of settings.
    let hint = wire::connection_hint(false, &wire::Settings::relay());
    assert_eq!((hint[0], hint.len()), (4, 2 + 32));
    let mut w = Writer::new(message_type::REQUEST_SERVER_CONNECTION);
    w.bytes(&[1, 0, 0, 0, 2, 0, 0, 0])
        .guid(&wire::PROTOCOL_VERSION)
        .i32(7);
    assert_eq!(
        wire::parse_client(&w.finish()).unwrap(),
        ClientMessage::RequestConnection {
            user_data: &[1, 0, 0, 0, 2, 0, 0, 0],
            protocol: wire::PROTOCOL_VERSION,
            internal_version: 7
        }
    );
    let mut w = Writer::rmi(rmi::P2P_GROUP_MEMBER_JOIN_ACK);
    w.i32(9).i32(12).i32(3).bool(false);
    assert_eq!(
        wire::parse_client(&w.finish()).unwrap(),
        ClientMessage::MemberJoinAck {
            group: 9,
            member: 12,
            event_id: 3
        }
    );
    // {0AFA3C0C-77D7-4B74-9DDB-1CB3D25E1E63} (1.69 EN client) in native layout.
    assert_eq!(&wire::PROTOCOL_VERSION[..4], &[0x0c, 0x3c, 0xfa, 0x0a]);
}

#[test]
fn session_key_simpleblob_decrypts_with_the_client_private_key() {
    let private = RsaPrivateKey::new(&mut rand_core::OsRng, 1024).unwrap();
    let blob = encrypt_session_key(&public_key_blob(&private)).unwrap();
    // BLOBHEADER(SIMPLEBLOB, 2, 0, CALG_RC4) + CALG_RSA_KEYX + little-endian ciphertext.
    assert_eq!(&blob[..4], &[1, 2, 0, 0]);
    assert_eq!(&blob[4..8], &0x6801_u32.to_le_bytes());
    assert_eq!(&blob[8..12], &0xa400_u32.to_le_bytes());
    let ciphertext: Vec<u8> = blob[12..].iter().rev().copied().collect();
    assert_eq!(ciphertext.len(), 128);
    let session = private.decrypt(Pkcs1v15Encrypt, &ciphertext).unwrap();
    assert_eq!(session.len(), 16);
    assert!(encrypt_session_key(&[6, 2, 0, 0]).is_none());
}

async fn read_message(s: &mut TcpStream, buf: &mut Vec<u8>) -> Vec<u8> {
    loop {
        if let Some((m, used)) = wire::extract(buf).unwrap() {
            buf.drain(..used);
            return m;
        }
        let mut chunk = [0_u8; 4096];
        let n = tokio::time::timeout(Duration::from_secs(5), s.read(&mut chunk))
            .await
            .unwrap()
            .unwrap();
        assert!(n > 0, "relay closed");
        buf.extend_from_slice(&chunk[..n]);
    }
}
async fn send(s: &mut TcpStream, payload: &[u8]) {
    let mut out = Vec::new();
    wire::frame(payload, &mut out);
    s.write_all(&out).await.unwrap();
}

#[tokio::test]
async fn relay_admits_a_waiting_client_and_joins_it_to_a_one_member_group() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let relay = Relay::new(addr);
    let mut events = relay.subscribe();
    tokio::spawn(relay.clone().serve(listener));
    let waiting = relay.add_client_waiting(1_234_567, 42);
    // A wrong key is denied and the connection closes.
    let mut bad = TcpStream::connect(addr).await.unwrap();
    let mut bad_buf = Vec::new();
    assert_eq!(read_message(&mut bad, &mut bad_buf).await[0], 4);
    let mut data = Vec::new();
    data.extend_from_slice(&waiting.sn.to_le_bytes());
    data.extend_from_slice(&waiting.key.wrapping_add(1).to_le_bytes());
    let mut w = Writer::new(message_type::REQUEST_SERVER_CONNECTION);
    w.bytes(&data).guid(&wire::PROTOCOL_VERSION).i32(0);
    send(&mut bad, &w.finish()).await;
    assert_eq!(read_message(&mut bad, &mut bad_buf).await, [8]);
    // The relay consumed the waiting entry only on success: the right key still works.
    let mut s = TcpStream::connect(addr).await.unwrap();
    let mut buf = Vec::new();
    assert_eq!(read_message(&mut s, &mut buf).await[0], 4);
    let private = RsaPrivateKey::new(&mut rand_core::OsRng, 1024).unwrap();
    let mut w = Writer::new(message_type::ENCRYPTED_SESSION_KEY);
    w.bytes(&public_key_blob(&private));
    send(&mut s, &w.finish()).await;
    assert_eq!(read_message(&mut s, &mut buf).await[0], 6);
    let mut data = Vec::new();
    data.extend_from_slice(&waiting.sn.to_le_bytes());
    data.extend_from_slice(&waiting.key.to_le_bytes());
    let mut w = Writer::new(message_type::REQUEST_SERVER_CONNECTION);
    w.bytes(&data).guid(&wire::PROTOCOL_VERSION).i32(0);
    send(&mut s, &w.finish()).await;
    let ten = read_message(&mut s, &mut buf).await;
    let mut r = Reader::new(&ten);
    assert_eq!(r.u8().unwrap(), 10);
    let host = r.i32().unwrap();
    let _instance = r.guid().unwrap();
    let reply = r.bytes(64).unwrap();
    assert_eq!(&reply[..8], &1_234_567_i64.to_le_bytes());
    assert_eq!(&reply[8..], &42_i32.to_le_bytes());
    // Not connected before NMInfosResult.
    let group = relay.make_group();
    assert!(!relay.join(
        group,
        1_234_567,
        77,
        "MicroPlayServiceCore.MicroPlayService"
    ));
    let mut w = Writer::rmi(rmi::NM_INFOS_RESULT);
    w.i64(1_234_567);
    send(&mut s, &w.finish()).await;
    assert_eq!(
        events.recv().await.unwrap(),
        RelayEvent::Connected {
            cid: 1_234_567,
            host_id: host
        }
    );
    assert!(relay.join(
        group,
        1_234_567,
        77,
        "MicroPlayServiceCore.MicroPlayService"
    ));
    let join = read_message(&mut s, &mut buf).await;
    let mut r = Reader::new(&join);
    assert_eq!((r.u8().unwrap(), r.u16().unwrap()), (1, 0xfbf6));
    assert_eq!((r.i32().unwrap(), r.i32().unwrap()), (group, host));
    let nm = read_message(&mut s, &mut buf).await;
    let mut r = Reader::new(&nm);
    assert_eq!((r.u8().unwrap(), r.u16().unwrap()), (1, 0x7d1));
    let _id = r.i64().unwrap();
    assert_eq!(r.i32().unwrap(), 1);
    assert_eq!(
        (r.i32().unwrap(), r.i64().unwrap(), r.i32().unwrap()),
        (host, 1_234_567, 42)
    );
    let mut w = Writer::rmi(rmi::P2P_GROUP_MEMBER_JOIN_ACK);
    w.i32(group).i32(host).i32(1).bool(false);
    send(&mut s, &w.finish()).await;
    assert_eq!(
        events.recv().await.unwrap(),
        RelayEvent::Joined {
            group,
            cid: 1_234_567
        }
    );
    let mut w = Writer::rmi(rmi::RELIABLE_PING);
    w.i32(0);
    send(&mut s, &w.finish()).await;
    assert_eq!(read_message(&mut s, &mut buf).await, [1, 0xfc, 0xfb]);
    drop(s);
    assert_eq!(
        events.recv().await.unwrap(),
        RelayEvent::Closed { cid: 1_234_567 }
    );
}
