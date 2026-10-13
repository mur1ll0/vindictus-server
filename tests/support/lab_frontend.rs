use super::{cleanup_game_pool, isolated_game_pool};
#[path = "lab_frontend_common.rs"]
mod common;
use common::*;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use vindictus_server::{
    application::game_users::provision_game_user,
    infrastructure::lab_frontend::Runtime,
    protocol::{
        DEFAULT_MAX_BODY, Frame, empty_character_list as list,
        frontend_control::{self as ctl, Kind as CKind, Message as CMessage},
        login_failure::decode_login_failure,
        second_password::{self as pwd, Kind as PKind, Message as PMessage},
        server_profile::LOGIN_FAIL_MESSAGE_CATEGORY,
    },
};
async fn lobby(
    client: &mut tokio::net::TcpStream,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
) -> pwd::HasPassword {
    let (mut has, mut got_list) = (None, false);
    while has.is_none() || !got_list {
        let f = read(client, rx).await;
        match f.category_id {
            413 => {
                let Some(PMessage::Has(q)) =
                    pwd::decode(PKind::Has, &f.body, DEFAULT_MAX_BODY).unwrap()
                else {
                    panic!("has")
                };
                has = Some(q);
            }
            479 => {
                let q = list::decode(&f.body, DEFAULT_MAX_BODY).unwrap().unwrap();
                assert_eq!(q.max_free, 20);
                assert_eq!(q.max_purchased, 0);
                assert_eq!(q.prologue_played, 0);
                got_list = true;
            }
            231 => {
                assert!(matches!(
                    pwd::decode(PKind::Ask, &f.body, DEFAULT_MAX_BODY).unwrap(),
                    Some(PMessage::Ask)
                ));
            }
            _ => panic!("unexpected recovered response category"),
        }
    }
    has.unwrap()
}
async fn password(
    client: &mut tokio::net::TcpStream,
    tx: &mut vindictus_server::protocol::transport::LegacySender,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
    message: PMessage<'_>,
) -> pwd::PasswordResult {
    let kind = message.kind();
    let body = pwd::encode(kind, Some(&message), DEFAULT_MAX_BODY).unwrap();
    send(client, tx, kind.category(), &body).await;
    let f = read(client, rx).await;
    assert_eq!(f.category_id, 562);
    let Some(PMessage::Result(r)) = pwd::decode(PKind::Result, &f.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("result")
    };
    r
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn actual_tcp_scoped_login_password_reconnect_duplicate_and_cleanup_persist() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let r = runtime();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    let ng = ctl::encode(
        CKind::Ng,
        Some(&CMessage::Ng {
            data: None,
            checksum: 0,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    let mut batch = tx
        .encode(&Frame {
            instance_id: 0,
            category_id: CKind::Ng.category(),
            body: &ng,
        })
        .unwrap();
    batch.extend(login(PASSPORT, &mut tx));
    for chunk in batch.chunks(7) {
        client.write_all(chunk).await.unwrap();
    }
    let q = lobby(&mut client, &mut rx).await;
    assert!(q.first && !q.has_password && q.passed);
    assert_eq!(r.active_count().unwrap(), 1);
    let before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_lab_account_origins")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, 1);
    let x = password(&mut client, &mut tx, &mut rx, PMessage::Set(Some("123456"))).await;
    assert_eq!(x.operation, 1);
    assert!(x.passed);
    assert_eq!(x.fail_count, 0);
    let (plain, length): (bool, i32) = sqlx::query_as(
        "SELECT second_password_utf16=$1,length(second_password_utf16) FROM game_users",
    )
    .bind(
        "123456"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>(),
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!plain);
    assert!(length > 0);
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 0);
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let q = lobby(&mut client, &mut rx).await;
    assert!(!q.first && q.has_password && !q.passed);
    let good = password(
        &mut client,
        &mut tx,
        &mut rx,
        PMessage::Check(Some("123456")),
    )
    .await;
    assert!(good.passed);
    assert_eq!(good.operation, 0);
    let (mut other, other_task, stop, mut other_rx, mut other_tx) =
        open(r.clone(), pool.clone()).await;
    other
        .write_all(&login(PASSPORT, &mut other_tx))
        .await
        .unwrap();
    let f = read(&mut other, &mut other_rx).await;
    assert_eq!(f.category_id, LOGIN_FAIL_MESSAGE_CATEGORY);
    assert_eq!(
        decode_login_failure(&f.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap()
            .reason,
        2
    );
    stop.send(true).unwrap();
    assert!(other_task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 1);
    // Candidate uses legacy mode0: NULL is incorrect and consumes a real fail count.
    let bad = password(&mut client, &mut tx, &mut rx, PMessage::Check(None)).await;
    assert!(!bad.passed);
    assert_eq!(bad.fail_count, 1);
    assert_eq!(bad.retry_seconds, -1);
    let good = password(
        &mut client,
        &mut tx,
        &mut rx,
        PMessage::Check(Some("123456")),
    )
    .await;
    assert!(good.passed);
    let bad = password(&mut client, &mut tx, &mut rx, PMessage::Set(None)).await;
    assert!(!bad.passed);
    assert_eq!(bad.operation, 1);
    assert_eq!(bad.fail_count, -1);
    assert!((590..=600).contains(&bad.retry_seconds));
    let body = pwd::encode(
        PKind::Check,
        Some(&PMessage::Check(Some("123456"))),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    send(&mut client, &mut tx, PKind::Check.category(), &body).await;
    let f = read(&mut client, &mut rx).await;
    assert_eq!(f.category_id, 602);
    let Some(CMessage::System { category, text }) =
        ctl::decode(CKind::System, &f.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("system")
    };
    assert_eq!(category, 4);
    assert_eq!(
        text.unwrap().format,
        Some("GameUI_Heroes_SecondPassword_Locked")
    );
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_lab_account_origins")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn actual_tcp_unmarked_user_refusal_has_no_effects_or_origin_and_releases_reservation() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[]);
    let p = profile(&c, false);
    let id = vindictus_server::protocol::identity::derive_lab_identity(
        Some(PASSPORT),
        p.identity_options(),
    )
    .unwrap()
    .nexon_sn;
    provision_game_user(&pool, id, PASSPORT, 42, 0)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let r = std::sync::Arc::new(Runtime::new(p, c).unwrap());
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let f = read(&mut client, &mut rx).await;
    assert_eq!(
        decode_login_failure(&f.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap()
            .reason,
        0
    );
    assert!(task.await.unwrap().is_err());
    assert_eq!(r.active_count().unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT local_revision FROM game_users WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        before
    );
    for table in [
        "game_lab_account_origins",
        "game_login_reward_runs",
        "game_login_account_effect_runs",
        "game_user_info",
        "game_user_care_info",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(&pool)
                .await
                .unwrap(),
            0
        );
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn actual_tcp_nonempty_character_dependency_sends_source_reason9_null_and_delayed_disconnect()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    let r = runtime();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    lobby(&mut client, &mut rx).await;
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    let uid: i32 = sqlx::query_scalar("SELECT id FROM game_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut b = vindictus_server::infrastructure::character_export::read_export(
        include_bytes!("../fixtures/character-bundle.json").as_slice(),
    )
    .unwrap();
    for c in &mut b.character_info {
        c.uid = uid;
    }
    vindictus_server::infrastructure::characters::import_bundle(&pool, &b, false)
        .await
        .unwrap();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    let now = std::time::Instant::now();
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let f = read(&mut client, &mut rx).await;
    assert_eq!(f.category_id, LOGIN_FAIL_MESSAGE_CATEGORY);
    let x = decode_login_failure(&f.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap();
    assert_eq!(x.reason, 9);
    assert_eq!(x.banned_reason, None);
    let mut byte = [0u8; 1];
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(8), client.read(&mut byte))
            .await
            .unwrap()
            .unwrap(),
        0
    );
    assert!(now.elapsed() >= Duration::from_secs(5));
    assert!(task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 0);
    cleanup_game_pool(admin, pool, schema).await;
}
