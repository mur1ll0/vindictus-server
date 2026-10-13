#[path = "support/lab_frontend_common.rs"]
mod common;
use common::*;
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vindictus_server::{
    infrastructure::lab_frontend::Runtime,
    protocol::{
        DEFAULT_MAX_BODY, Frame,
        frontend_control::{self as ctl, Kind, Message},
        login_failure::decode_login_failure,
        server_profile::LOGIN_FAIL_MESSAGE_CATEGORY,
    },
};
fn no_database() -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(50))
        .connect_lazy("postgres://synthetic:synthetic@127.0.0.1:1/synthetic")
        .unwrap()
}
#[test]
fn runtime_refuses_active_unported_dependencies_and_enabled_ping_before_network_or_database() {
    for key in [
        "PassportImmigrator",
        "CafeBonus",
        "CashShop",
        "FraudDetector",
        "XignCode",
        "OldNexonIDProtect",
        "SecondPassword_IP_Limit",
    ] {
        let c = content(&[(key, Some("1"))]);
        assert!(Runtime::new(profile(&c, false), c).is_err(), "{key}");
    }
    let c = content(&[]);
    assert!(Runtime::new(profile(&c, true), c).is_err());
    let c = content(&[("HackshieldOff", None)]);
    assert!(Runtime::new(profile(&c, false), c).is_err());
    let c = content(&[("SecondPassword", None)]);
    assert!(Runtime::new(profile(&c, false), c).is_err());
}
#[tokio::test]
async fn batch_ng_and_unlisted_login_consumes_both_frames_without_database_access() {
    let r = runtime();
    let (mut client, task, stop, mut rx, mut tx) = open(r.clone(), no_database()).await;
    let body = ctl::encode(
        Kind::Ng,
        Some(&Message::Ng {
            data: None,
            checksum: 0,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    let mut batch = tx
        .encode(&Frame {
            instance_id: 0,
            category_id: Kind::Ng.category(),
            body: &body,
        })
        .unwrap();
    batch.extend(login("unlisted_synthetic", &mut tx));
    client.write_all(&batch).await.unwrap();
    let failure = read(&mut client, &mut rx).await;
    assert_eq!(failure.category_id, LOGIN_FAIL_MESSAGE_CATEGORY);
    assert_eq!(
        decode_login_failure(&failure.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap()
            .reason,
        13
    );
    assert_eq!(r.active_count().unwrap(), 0);
    stop.send(true).unwrap();
    assert!(task.await.unwrap().is_ok());
}
#[tokio::test]
async fn fragmented_null_login_closes_without_reservation_or_database() {
    let r = runtime();
    let (mut client, task, _stop, _rx, mut tx) = open(r.clone(), no_database()).await;
    let bytes = tx
        .encode(&Frame {
            instance_id: 0,
            category_id: 478,
            body: &[0],
        })
        .unwrap();
    for b in bytes {
        client.write_all(&[b]).await.unwrap();
    }
    assert!(task.await.unwrap().is_err());
    assert_eq!(r.active_count().unwrap(), 0);
}
#[tokio::test]
async fn runtime_shutdown_drains_open_connection_and_incomplete_stream_is_rejected() {
    let r = runtime();
    let (mut client, task, stop, _rx, _tx) = open(r.clone(), no_database()).await;
    stop.send(true).unwrap();
    assert!(task.await.unwrap().is_ok());
    let mut b = [0u8; 1];
    assert_eq!(client.read(&mut b).await.unwrap(), 0);
    let (mut client, task, _stop, _rx, mut tx) = open(r.clone(), no_database()).await;
    let pong = ctl::encode(Kind::Pong, Some(&Message::Pong(0)), DEFAULT_MAX_BODY).unwrap();
    send(&mut client, &mut tx, Kind::Pong.category(), &pong).await;
    let bytes = login(PASSPORT, &mut tx);
    client.write_all(&bytes[..bytes.len() - 1]).await.unwrap();
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_err());
    assert_eq!(r.active_count().unwrap(), 0);
}
