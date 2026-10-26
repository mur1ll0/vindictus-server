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

#[path = "minimum_selection.rs"]
mod minimum_selection;
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_minimum_create_returns_persisted_filled_list_and_relogin() {
    minimum_flow(false).await;
}
async fn minimum_flow(with_channel: bool) {
    minimum_flow_equipment(with_channel, false).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_minimum_equipped_selection_channel_and_relogin() {
    minimum_flow_equipment(true, true).await;
}
async fn minimum_flow_equipment(with_channel: bool, with_equipment: bool) {
    use vindictus_server::protocol::{character_creation as create, character_list as filled};
    let (admin, pool, schema) = isolated_game_pool().await;
    super::minimum_character::setup(&pool).await;
    sqlx::query("DELETE FROM game_characters")
        .execute(&pool)
        .await
        .unwrap();
    let c = content(&[
        ("DeleteCharacterDelay", Some("1440")),
        ("DormantAccount", None),
    ]);
    let proof = super::name_check_common::proof(&c);
    let path = std::env::temp_dir().join(format!(
        "vindictus-minimum-proof-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(
        &path,
        serde_json::to_vec(&super::name_check_common::value(&c)).unwrap(),
    )
    .unwrap();
    let mut assets = match std::sync::Arc::try_unwrap(super::minimum_character::assets()) {
        Ok(a) => a,
        Err(_) => panic!("assets unexpectedly shared"),
    };
    if with_channel {
        // DEX=18 produces DEF=8 on Framework x86 and DEF=9 on x64.
        // Body arithmetic remains unambiguous-only; stats follow the frontend.
        assets.initializer =
            super::fiona_inventory_delivery::profile_with_base(false, [29, 18, 18, 43, 901, 140]);
        assets.channel = Some((
            "127.0.0.1:0".parse().unwrap(),
            minimum_selection::topology(),
        ));
    }
    if with_equipment {
        assets = assets
            .with_equipment(
                vindictus_server::infrastructure::equipment_profile::Profile::read(
                    serde_json::to_vec(&super::minimum_equipment::equipment_profile_fixture())
                        .unwrap()
                        .as_slice(),
                )
                .unwrap(),
            )
            .unwrap();
    }
    let runtime = Runtime::new(profile(&c, false), c)
        .unwrap()
        .with_name_check(Some(proof))
        .with_minimum_creation(pool.clone(), assets, &path)
        .unwrap();
    std::fs::remove_file(path).unwrap();
    let r = std::sync::Arc::new(runtime);
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    lobby(&mut client, &mut rx).await;
    assert!(
        password(&mut client, &mut tx, &mut rx, PMessage::Set(Some("123456")))
            .await
            .passed
    );
    let bytes = super::minimum_character::body();
    send(
        &mut client,
        &mut tx,
        create::Kind::Create.category(),
        &bytes,
    )
    .await;
    let response = read(&mut client, &mut rx).await;
    assert_eq!(response.category_id, 479);
    let decoded = filled::decode(&response.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap();
    assert_eq!(decoded.party_states.as_deref(), Some([0].as_slice()));
    let cid = decoded.characters.as_ref().unwrap()[0]
        .as_ref()
        .unwrap()
        .cid;
    if with_equipment {
        sqlx::query("UPDATE game_equippables SET part=1,color1=31,color2=32,color3=33 WHERE id=(SELECT min(e.id)FROM game_equippables e JOIN game_items i ON i.id=e.id WHERE i.owner_id=$1)").bind(cid).execute(&pool).await.unwrap();
    }
    assert_eq!(
        decoded.characters.as_ref().unwrap()[0]
            .as_ref()
            .unwrap()
            .delete_wait_left_sec,
        123 * 60
    );
    assert_eq!(
        decoded.characters.as_ref().unwrap()[0]
            .as_ref()
            .unwrap()
            .character_id,
        Some("Murael")
    );
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let (mut got_list, mut got_password) = (false, false);
    while !got_list || !got_password {
        let f = read(&mut client, &mut rx).await;
        match f.category_id {
            479 => {
                let d = filled::decode(&f.body, DEFAULT_MAX_BODY).unwrap().unwrap();
                assert_eq!(d.party_states.as_deref(), Some([0].as_slice()));
                assert_eq!(
                    d.characters.as_ref().unwrap().len(),
                    d.party_states.as_ref().unwrap().len()
                );
                assert_eq!(d.characters.unwrap()[0].as_ref().unwrap().cid, cid);
                got_list = true
            }
            413 => {
                got_password = true;
            }
            231 => {}
            _ => panic!("unexpected relogin response"),
        }
    }
    if with_channel {
        minimum_selection::locked_selection(&mut client, &mut rx, &mut tx).await;
    }
    assert!(
        password(
            &mut client,
            &mut tx,
            &mut rx,
            PMessage::Check(Some("123456"))
        )
        .await
        .passed
    );
    if with_channel {
        minimum_selection::exercise(&r, &pool, cid, &mut client, &mut rx, &mut tx).await;
    }
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    r.shutdown_minimum().await.unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}
async fn lobby(
    client: &mut tokio::net::TcpStream,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
) -> pwd::HasPassword {
    lobby_with_prologue(client, rx, 0).await
}
async fn lobby_with_prologue(
    client: &mut tokio::net::TcpStream,
    rx: &mut vindictus_server::protocol::transport::LegacyReceiver,
    expected: u8,
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
                assert_eq!(q.prologue_played, expected);
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

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_prologue_accepts_source_state_without_ack_and_reconnect_reads_persistence() {
    use vindictus_server::protocol::character_setup::{self as setup, Kind, Message};
    let (admin, pool, schema) = isolated_game_pool().await;
    let r = runtime();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    let body = setup::encode(Kind::Play, Some(&Message::Play(i32::MIN)), DEFAULT_MAX_BODY).unwrap();
    // Valid pre-admission play messages and nullable DTOs cannot create UserInfo.
    send(&mut client, &mut tx, Kind::Play.category(), &body).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    lobby(&mut client, &mut rx).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_user_info")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let body = setup::encode(Kind::Play, None, DEFAULT_MAX_BODY).unwrap();
    send(&mut client, &mut tx, Kind::Play.category(), &body).await;
    // Password response acts as the barrier, not an invented play acknowledgement.
    password(&mut client, &mut tx, &mut rx, PMessage::Set(Some("123456"))).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_user_info")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    for value in [i32::MIN, 0, i32::MAX] {
        let body =
            setup::encode(Kind::Play, Some(&Message::Play(value)), DEFAULT_MAX_BODY).unwrap();
        send(&mut client, &mut tx, Kind::Play.category(), &body).await;
    }
    password(
        &mut client,
        &mut tx,
        &mut rx,
        PMessage::Check(Some("123456")),
    )
    .await;
    assert_eq!(
        sqlx::query_as::<_, (Option<i64>, bool)>(
            "SELECT last_login_cid,prologue_played FROM game_user_info"
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        (None, true)
    );
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let has = lobby_with_prologue(&mut client, &mut rx, 1).await;
    assert!(has.has_password && !has.passed);
    // The source play branch does not require successful second-password validation.
    let body = setup::encode(Kind::Play, Some(&Message::Play(-1)), DEFAULT_MAX_BODY).unwrap();
    let mut unregistered = Kind::Play.type_id().as_bytes().to_vec();
    unregistered.extend_from_slice(&body);
    send(&mut client, &mut tx, 0, &unregistered).await;
    password(
        &mut client,
        &mut tx,
        &mut rx,
        PMessage::Check(Some("123456")),
    )
    .await;
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_character_info")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_reservation_query_returns_real_lists_before_password_without_consumption() {
    use vindictus_server::protocol::character_setup::{self as setup, Kind, Message};
    let (admin, pool, schema) = isolated_game_pool().await;
    let r = runtime();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    let body = setup::encode(Kind::Query, Some(&Message::Query), DEFAULT_MAX_BODY).unwrap();
    send(&mut client, &mut tx, Kind::Query.category(), &body).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    lobby(&mut client, &mut rx).await;
    send(&mut client, &mut tx, Kind::Query.category(), &body).await;
    let f = read(&mut client, &mut rx).await;
    assert_eq!(f.category_id, Kind::Reserved.category());
    let Some(Message::Reserved { names, titles }) =
        setup::decode(Kind::Reserved, &f.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("reserved info");
    };
    assert_eq!(names, Some(vec![]));
    assert_eq!(titles, Some(vec![]));
    password(&mut client, &mut tx, &mut rx, PMessage::Set(Some("123456"))).await;
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    let bytes: Vec<u8> = PASSPORT.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let name: Vec<u8> = "ReservedLab"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    sqlx::query("INSERT INTO game_reserved_names VALUES(1,$1,-99,$2,false)")
        .bind(&bytes)
        .bind(&name)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_reserved_titles VALUES(1,$1,-99,42,false)")
        .bind(&bytes)
        .execute(&pool)
        .await
        .unwrap();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let has = lobby(&mut client, &mut rx).await;
    assert!(has.has_password && !has.passed);
    let mut unregistered = Kind::Query.type_id().as_bytes().to_vec();
    unregistered.extend_from_slice(&body);
    send(&mut client, &mut tx, 0, &unregistered).await;
    let f = read(&mut client, &mut rx).await;
    assert_eq!(f.category_id, Kind::Reserved.category());
    let Some(Message::Reserved { names, titles }) =
        setup::decode(Kind::Reserved, &f.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("reserved info");
    };
    assert_eq!(names, Some(vec![Some("ReservedLab")]));
    assert_eq!(titles, Some(vec![42]));
    let body = setup::encode(Kind::Query, None, DEFAULT_MAX_BODY).unwrap();
    send(&mut client, &mut tx, Kind::Query.category(), &body).await;
    password(
        &mut client,
        &mut tx,
        &mut rx,
        PMessage::Check(Some("123456")),
    )
    .await;
    let flags:(bool,bool)=sqlx::query_as("SELECT (SELECT created FROM game_reserved_names WHERE row_id=1),(SELECT title_given FROM game_reserved_titles WHERE row_id=1)").fetch_one(&pool).await.unwrap();
    assert_eq!(flags, (false, false));
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    assert_eq!(r.active_count().unwrap(), 0);
    cleanup_game_pool(admin, pool, schema).await;
}
use super::name_check_common as naming_common;
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn actual_tcp_name_check_uses_source_dialog_and_name_change_result_before_second_password() {
    use std::sync::Arc;
    use vindictus_server::protocol::character_setup::{
        self as setup, Kind as SKind, Message as SMessage,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let c = content(&[]);
    let proof = naming_common::proof(&c);
    let r = Arc::new(
        Runtime::new(profile(&c, false), c)
            .unwrap()
            .with_name_check(Some(proof)),
    );
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    let bytes = setup::encode(
        SKind::Check,
        Some(&SMessage::Check {
            name: Some("Murael"),
            name_change: false,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    send(&mut client, &mut tx, SKind::Check.category(), &bytes).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    lobby(&mut client, &mut rx).await;
    send(&mut client, &mut tx, SKind::Check.category(), &bytes).await;
    let frame = read(&mut client, &mut rx).await;
    assert_eq!(frame.category_id, CKind::System.category());
    let message = ctl::decode(CKind::System, &frame.body, DEFAULT_MAX_BODY).unwrap();
    assert!(
        matches!(message,Some(CMessage::System{category:4,text:Some(ctl::HeroesText{format:Some("GameUI_Heroes_Create_Character_ValidName"),parameters:Some(ref p)})}) if p.is_empty())
    );
    assert!(
        password(&mut client, &mut tx, &mut rx, PMessage::Set(Some("123456")))
            .await
            .passed
    );
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let password_state = lobby(&mut client, &mut rx).await;
    assert!(password_state.has_password && !password_state.passed);
    for (name, name_change, valid, error) in [
        ("Murael", true, true, None),
        (
            "A!",
            false,
            false,
            Some("GameUI_Heroes_Error_Create_Character_InvalidName"),
        ),
        (
            "A!",
            true,
            false,
            Some("GameUI_Heroes_Error_Create_Character_InvalidName"),
        ),
    ] {
        let body = setup::encode(
            SKind::Check,
            Some(&SMessage::Check {
                name: Some(name),
                name_change,
            }),
            DEFAULT_MAX_BODY,
        )
        .unwrap();
        send(&mut client, &mut tx, SKind::Check.category(), &body).await;
        let frame = read(&mut client, &mut rx).await;
        if name_change {
            assert_eq!(frame.category_id, SKind::Result.category());
            assert!(
                setup::decode(SKind::Result, &frame.body, DEFAULT_MAX_BODY).unwrap()
                    == Some(SMessage::Result {
                        valid,
                        name: Some(name),
                        error
                    })
            );
        } else {
            assert_eq!(frame.category_id, CKind::System.category());
            assert!(
                ctl::decode(CKind::System, &frame.body, DEFAULT_MAX_BODY).unwrap()
                    == Some(CMessage::System {
                        category: 4,
                        text: Some(ctl::HeroesText {
                            format: error,
                            parameters: Some(Vec::new())
                        })
                    })
            );
        }
    }
    // An unmeasured valid name receives no manufactured availability result; Query is a barrier.
    let body = setup::encode(
        SKind::Check,
        Some(&SMessage::Check {
            name: Some("OtherName"),
            name_change: false,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    send(&mut client, &mut tx, SKind::Check.category(), &body).await;
    let query = setup::encode(SKind::Query, Some(&SMessage::Query), DEFAULT_MAX_BODY).unwrap();
    send(&mut client, &mut tx, SKind::Query.category(), &query).await;
    let frame = read(&mut client, &mut rx).await;
    assert_eq!(frame.category_id, SKind::Reserved.category());
    sqlx::query("INSERT INTO game_character_name_logs(id,old_character_name_utf16,new_character_name_utf16) VALUES(1,$1,$2)").bind("Murael".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()).bind("OtherName".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()).execute(&pool).await.unwrap();
    let body = setup::encode(
        SKind::Check,
        Some(&SMessage::Check {
            name: Some("Murael"),
            name_change: false,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    send(&mut client, &mut tx, SKind::Check.category(), &body).await;
    let frame = read(&mut client, &mut rx).await;
    assert!(
        ctl::decode(CKind::System, &frame.body, DEFAULT_MAX_BODY).unwrap()
            == Some(CMessage::System {
                category: 4,
                text: Some(ctl::HeroesText {
                    format: Some("GameUI_Heroes_Error_Create_Character_ChangedCharacterName"),
                    parameters: Some(Vec::new())
                })
            })
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_character_info")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    client.shutdown().await.unwrap();
    assert!(task.await.unwrap().is_ok());
    cleanup_game_pool(admin, pool, schema).await;
}
/// B4.2: an allowlist name claimed by an account no longer logs in by its passport alone, and
/// the refusal reserves no session and creates no game user.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn actual_tcp_allowlist_name_claimed_by_an_account_is_refused() {
    use vindictus_server::{
        application::{accounts::create_account, admin_session::link_game_from_cli},
        domain::{Username, passport::LoginRejection},
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    create_account(
        &pool,
        Username::parse("Dono").unwrap(),
        zeroize::Zeroizing::new(b"senha de teste do jogo".to_vec()),
    )
    .await
    .unwrap();
    link_game_from_cli(
        &pool,
        "dono",
        PASSPORT,
        "conta ligada ao usuário de laboratório",
    )
    .await
    .unwrap();
    let r = runtime();
    let (mut client, task, _stop, mut rx, mut tx) = open(r.clone(), pool.clone()).await;
    client.write_all(&login(PASSPORT, &mut tx)).await.unwrap();
    let f = read(&mut client, &mut rx).await;
    assert_eq!(
        decode_login_failure(&f.body, DEFAULT_MAX_BODY)
            .unwrap()
            .unwrap()
            .reason,
        LoginRejection::PermissionFailed as i32
    );
    drop(client);
    let _ = task.await;
    assert_eq!(r.active_count().unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_users")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    cleanup_game_pool(admin, pool, schema).await;
}
