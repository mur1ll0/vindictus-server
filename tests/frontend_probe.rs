use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    task::JoinHandle,
};
use vindictus_server::{
    infrastructure::frontend::{ProbeError, ProbeOutcome, probe_connection, run_probe},
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        bootstrap::encode_initial_packets,
        converter::decode_converter,
        features::{FeatureEntry, FeatureMatrix},
        login_failure::{PERMISSION_FAILED, decode_login_failure},
        server_profile::{LOGIN_FAIL_MESSAGE_CATEGORY, USER_LOGIN_MESSAGE_CATEGORY},
        transport::{CipherProfile, LegacyReceiver, LegacySender, OwnedFrame},
    },
};
const TEST_DEADLINE: Duration = Duration::from_secs(3);
async fn connection(
    deadline: Duration,
    with_features: bool,
) -> (TcpStream, JoinHandle<Result<ProbeOutcome, ProbeError>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap())
        .await
        .unwrap();
    let (server, _) = listener.accept().await.unwrap();
    let task = tokio::spawn(async move {
        let features = FeatureMatrix {
            features: Some(vec![
                FeatureEntry {
                    key: "NGM_disable",
                    value: Some("1"),
                },
                FeatureEntry {
                    key: "TestUnicode",
                    value: Some("É-λ"),
                },
            ]),
        };
        probe_connection(
            server,
            169,
            if with_features { Some(&features) } else { None },
            deadline,
        )
        .await
    });
    (client, task)
}
async fn read_frame(stream: &mut TcpStream, receiver: &mut LegacyReceiver) -> OwnedFrame {
    tokio::time::timeout(TEST_DEADLINE, async {
        let mut byte = [0; 1];
        loop {
            stream.read_exact(&mut byte).await.unwrap();
            let mut frames = receiver.push(&byte).unwrap();
            if !frames.is_empty() {
                return frames.remove(0);
            }
        }
    })
    .await
    .unwrap()
}
fn login_body() -> Vec<u8> {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/legacy-packet.json")).unwrap();
    fixture["login"][1]["body"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
#[tokio::test]
async fn real_tcp_fragmented_login_is_refused_and_outgoing_cipher_remains_continuous() {
    let (mut client, task) = connection(TEST_DEADLINE, true).await;
    let mut receiver =
        LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
    let registration = read_frame(&mut client, &mut receiver).await;
    assert_eq!(registration.category_id, 0);
    assert_eq!(
        decode_converter(&registration.body, DEFAULT_MAX_BODY, 1024)
            .unwrap()
            .entries()
            .count(),
        649
    );
    let features = read_frame(&mut client, &mut receiver).await;
    assert_eq!(features.category_id, 185);
    let mut sender =
        LegacySender::with_profile(CipherProfile::VindictusFrontend, 123, DEFAULT_MAX_BODY)
            .unwrap();
    let bytes = sender
        .encode(&Frame {
            instance_id: 0,
            category_id: USER_LOGIN_MESSAGE_CATEGORY,
            body: &login_body(),
        })
        .unwrap();
    for chunk in bytes.chunks(3) {
        client.write_all(chunk).await.unwrap();
    }
    let refusal = read_frame(&mut client, &mut receiver).await;
    assert_eq!(refusal.category_id, LOGIN_FAIL_MESSAGE_CATEGORY);
    let refusal = decode_login_failure(&refusal.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap();
    assert_eq!(refusal.reason, PERMISSION_FAILED);
    assert_eq!(refusal.banned_reason, Some(""));
    assert_eq!(
        tokio::time::timeout(TEST_DEADLINE, task)
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        ProbeOutcome::LoginRefused
    );
    let mut byte = [0];
    assert_eq!(client.read(&mut byte).await.unwrap(), 0);
}
#[tokio::test]
async fn default_probe_bootstrap_is_byte_identical_to_the_verified_builder() {
    let (mut client, task) = connection(TEST_DEADLINE, false).await;
    let (_, packets) = encode_initial_packets(169, None, DEFAULT_MAX_BODY, 1024).unwrap();
    let mut bytes = vec![0; packets[0].len()];
    client.read_exact(&mut bytes).await.unwrap();
    assert_eq!(bytes, packets[0]);
    client.shutdown().await.unwrap();
    assert_eq!(task.await.unwrap().unwrap(), ProbeOutcome::PeerClosed);
}
#[tokio::test]
async fn partial_packet_eof_and_null_login_cannot_become_an_authenticated_session() {
    for body in [vec![0], vec![1, 2]] {
        let (mut client, task) = connection(TEST_DEADLINE, false).await;
        let mut receiver =
            LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
        read_frame(&mut client, &mut receiver).await;
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123, DEFAULT_MAX_BODY)
                .unwrap();
        let bytes = sender
            .encode(&Frame {
                instance_id: 0,
                category_id: USER_LOGIN_MESSAGE_CATEGORY,
                body: &body,
            })
            .unwrap();
        if body == [0] {
            client.write_all(&bytes).await.unwrap();
            assert!(matches!(task.await.unwrap(), Err(ProbeError::NullLogin)));
        } else {
            client.write_all(&bytes[..5]).await.unwrap();
            client.shutdown().await.unwrap();
            assert!(matches!(task.await.unwrap(), Err(ProbeError::TruncatedEof)));
        }
    }
}
#[tokio::test]
async fn unexpected_typed_categories_are_not_negotiated_as_trusted_login() {
    for category_id in [1, -1, 649] {
        let (mut client, task) = connection(TEST_DEADLINE, false).await;
        let mut receiver =
            LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
        read_frame(&mut client, &mut receiver).await;
        let mut sender =
            LegacySender::with_profile(CipherProfile::VindictusFrontend, 123, DEFAULT_MAX_BODY)
                .unwrap();
        client
            .write_all(
                &sender
                    .encode(&Frame {
                        instance_id: 0,
                        category_id,
                        body: &[1],
                    })
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            matches!(task.await.unwrap(), Err(ProbeError::UnsupportedCategory(value)) if value == category_id)
        );
    }
}
#[tokio::test]
async fn absolute_deadline_closes_a_peer_that_never_finishes_a_packet() {
    let (mut client, task) = connection(Duration::from_millis(300), false).await;
    let mut receiver =
        LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
    read_frame(&mut client, &mut receiver).await;
    client.write_all(&[0]).await.unwrap();
    assert!(matches!(
        tokio::time::timeout(TEST_DEADLINE, task)
            .await
            .unwrap()
            .unwrap(),
        Err(ProbeError::Timeout)
    ));
}
#[tokio::test]
async fn shutdown_aborts_pending_connections_and_releases_listening_address() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(run_probe(listener, async {
        let _ = stopped.await;
    }));
    let mut client = TcpStream::connect(address).await.unwrap();
    let mut receiver =
        LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
    read_frame(&mut client, &mut receiver).await;
    stop.send(()).unwrap();
    tokio::time::timeout(TEST_DEADLINE, task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let mut byte = [0];
    assert_eq!(
        tokio::time::timeout(TEST_DEADLINE, client.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert!(TcpStream::connect(address).await.is_err());
}
#[tokio::test]
async fn recovered_framework_category_zero_login_reaches_the_same_explicit_refusal() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-login-envelope.json")).unwrap();
    let case = &fixture["profiles"][0]["cases"][9];
    assert_eq!(case["category"], 0);
    let wire: Vec<u8> = case["wire"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
        .collect();
    let (mut client, task) = connection(TEST_DEADLINE, false).await;
    let mut receiver =
        LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
    read_frame(&mut client, &mut receiver).await;
    for byte in wire {
        client.write_all(&[byte]).await.unwrap();
    }
    assert_eq!(task.await.unwrap().unwrap(), ProbeOutcome::LoginRefused);
    let response = read_frame(&mut client, &mut receiver).await;
    assert_eq!(response.category_id, LOGIN_FAIL_MESSAGE_CATEGORY);
    assert_eq!(
        decode_login_failure(&response.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap()
            .reason,
        PERMISSION_FAILED
    );
}
#[tokio::test]
async fn category_zero_null_and_peer_converter_cannot_start_a_login() {
    use vindictus_server::protocol::login_ingress::LoginIngressError;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-login-envelope.json")).unwrap();
    let null_wire: Vec<u8> = fixture["profiles"][0]["cases"][0]["wire"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| u8::try_from(v.as_u64().unwrap()).unwrap())
        .collect();
    let (_, converter_wire) = encode_initial_packets(169, None, DEFAULT_MAX_BODY, 1024).unwrap();
    for (wire, null) in [(null_wire, true), (converter_wire[0].clone(), false)] {
        let (mut client, task) = connection(TEST_DEADLINE, false).await;
        let mut receiver =
            LegacyReceiver::with_profile(CipherProfile::VindictusFrontend, DEFAULT_MAX_BODY);
        read_frame(&mut client, &mut receiver).await;
        client.write_all(&wire).await.unwrap();
        let result = task.await.unwrap();
        if null {
            assert!(matches!(result, Err(ProbeError::NullLogin)));
        } else {
            assert!(matches!(
                result,
                Err(ProbeError::Ingress(LoginIngressError::UnsupportedType))
            ));
        }
        let mut byte = [0];
        assert_eq!(client.read(&mut byte).await.unwrap(), 0);
    }
}
