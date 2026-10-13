use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::{character_lobby::*, legacy_user_data::*},
    infrastructure::{
        character_lobby::{self as storage, *},
        lobby_export::read_snapshot,
    },
};
fn snapshot() -> LobbySnapshot {
    read_snapshot(include_bytes!("../fixtures/synthetic-character-lobby.json").as_slice()).unwrap()
}
fn config(care: bool, time: i64) -> QueryConfig {
    QueryConfig {
        max_free: 20,
        care_clock: if care {
            Some(SqlDateTime::from_units300(time).unwrap())
        } else {
            None
        },
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn lobby_preservation_compares_every_field_replays_and_rolls_back_late_conflict() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let data = snapshot();
    let r = preserve(&pool, &data, false).await.unwrap();
    assert_eq!(r.inserted, [2, 1, 1]);
    assert_eq!(r.verified, [2, 1, 1]);
    assert_eq!(
        preserve(&pool, &data, false).await.unwrap().inserted,
        [0, 0, 0]
    );
    assert_eq!(
        preserve(&pool, &data, true).await.unwrap().verified,
        [2, 1, 1]
    );
    let mut conflict = snapshot();
    conflict.user_info.push(UserInfo {
        nexon_sn: 13,
        last_login_cid: Some(i64::MAX),
        prologue_played: false,
    });
    conflict.user_products[0].argument = None;
    assert!(matches!(
        preserve(&pool, &conflict, false).await,
        Err(LobbyError::Conflict)
    ));
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_user_info WHERE nexon_sn=13")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    assert_eq!(
        preserve(&pool, &data, true).await.unwrap().verified,
        [2, 1, 1]
    );
    // No inferred FK: the source keys/CIDs remain preservable without matching game users/characters.
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn lobby_query_reads_real_defaults_slots_expired_products_and_care_uid_upsert() {
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 71, "LobbyLab", 1, 0)
        .await
        .unwrap();
    let revision: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=71")
        .fetch_one(&pool)
        .await
        .unwrap();
    let key = || UserKey {
        uid: 71,
        name: "LOBBYLAB ",
        revision,
    };
    let r = query_empty(&pool, key(), config(true, 0)).await.unwrap();
    assert_eq!(
        (
            r.last_cid,
            r.prologue_played,
            r.max_free,
            r.max_purchased,
            r.max_premium,
            r.preset_used
        ),
        (0, false, 20, 0, 0, 0)
    );
    let c: (i16, i16, i64) = sqlx::query_as(
        "SELECT kind,state,last_update_units300 FROM game_user_care_info WHERE uid=71",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(c, (1, 2, 0));
    sqlx::query(
        "INSERT INTO game_user_info(nexon_sn,last_login_cid,prologue_played) VALUES(71,$1,true)",
    )
    .bind(i64::MAX)
    .execute(&pool)
    .await
    .unwrap();
    for name in [
        "CharacterSlot",
        "CHARACTERSLOT   ",
        "PremiumCharacterSlot",
        "other",
    ] {
        let bytes = name
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        sqlx::query("INSERT INTO game_user_products(nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks) VALUES(71,$1,NULL,0)").bind(bytes).execute(&pool).await.unwrap();
    }
    let r = query_empty(&pool, key(), config(false, 999)).await.unwrap();
    assert_eq!(
        (
            r.last_cid,
            r.prologue_played,
            r.max_purchased,
            r.max_premium
        ),
        (i64::MAX, true, 2, 1)
    );
    let c: (i16, i16, i64) = sqlx::query_as(
        "SELECT kind,state,last_update_units300 FROM game_user_care_info WHERE uid=71",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(c, (1, 2, 0));
    sqlx::query("UPDATE game_user_info SET last_login_cid=NULL WHERE nexon_sn=71")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE game_user_care_info SET kind=255,state=-32768,last_update_units300=-1 WHERE uid=71",
    )
    .execute(&pool)
    .await
    .unwrap();
    let r = query_empty(&pool, key(), config(true, 44)).await.unwrap();
    assert_eq!(r.last_cid, 0);
    let c: (i16, i16, i64) = sqlx::query_as(
        "SELECT kind,state,last_update_units300 FROM game_user_care_info WHERE uid=71",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(c, (1, 2, 44));
    assert!(matches!(
        query_empty(
            &pool,
            UserKey {
                uid: 71,
                name: "DifferentLab",
                revision
            },
            config(true, 55)
        )
        .await,
        Err(LobbyError::Ownership)
    ));
    assert!(matches!(
        query_empty(
            &pool,
            UserKey {
                uid: 71,
                name: "LobbyLab",
                revision: revision + 1
            },
            config(true, 55)
        )
        .await,
        Err(LobbyError::Ownership)
    ));
    let bytes = vec![233u8, 0];
    sqlx::query("INSERT INTO game_user_products(nexon_sn,product_name_utf16) VALUES(71,$1)")
        .bind(bytes)
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        query_empty(&pool, key(), config(true, 55)).await,
        Err(LobbyError::Value(LobbyValueError::ProductName))
    ));
    let after: i64 =
        sqlx::query_scalar("SELECT last_update_units300 FROM game_user_care_info WHERE uid=71")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, 44);
    sqlx::query("DELETE FROM game_user_products WHERE nexon_sn=71")
        .execute(&pool)
        .await
        .unwrap();
    let mut b = vindictus_server::infrastructure::character_export::read_export(
        include_bytes!("../fixtures/character-bundle.json").as_slice(),
    )
    .unwrap();
    for c in &mut b.character_info {
        c.uid = 71;
    }
    vindictus_server::infrastructure::characters::import_bundle(&pool, &b, false)
        .await
        .unwrap();
    assert!(matches!(
        query_empty(&pool, key(), config(true, 55)).await,
        Err(LobbyError::Characters)
    ));
    let after: i64 =
        sqlx::query_scalar("SELECT last_update_units300 FROM game_user_care_info WHERE uid=71")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after, 44);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn lobby_concurrent_imports_have_one_creator_and_cancelled_wait_writes_nothing() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let pool = pool.clone();
        tasks.spawn(async move { preserve(&pool, &snapshot(), false).await.unwrap().inserted });
    }
    let mut answers = Vec::new();
    while let Some(r) = tasks.join_next().await {
        answers.push(r.unwrap());
    }
    answers.sort();
    assert_eq!(answers, vec![[0, 0, 0], [2, 1, 1]]);
    provision_game_user(&pool, 72, "CancelLobby", 1, 0)
        .await
        .unwrap();
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    let request = query_empty(
        &pool,
        UserKey {
            uid: 72,
            name: "CancelLobby",
            revision: 0,
        },
        config(true, 123),
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), request)
            .await
            .is_err()
    );
    drop(held);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_user_care_info WHERE uid=72")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    query_empty(
        &pool,
        UserKey {
            uid: 72,
            name: "CancelLobby",
            revision: 0,
        },
        config(true, 123),
    )
    .await
    .unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn lobby_application_requires_admitted_owner_complete_policy_and_ready_schema() {
    use vindictus_server::application::{
        character_lobby::{LobbyQueryError, query_empty_admitted},
        frontend_session::FrontendSession,
        lab_login::{PolicyClock, evaluate_existing_lab_user},
        session_registry::SessionRegistry,
    };
    use vindictus_server::domain::{
        feature_policy::FeaturePolicy, login_policy::ApplicationClockSamples,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(8).unwrap();
    let mut f = FrontendSession::new(&registry);
    let now = SqlDateTime2::from_ticks(638000000000000000).unwrap();
    let policy = FeaturePolicy::new(
        [
            ("CurrentVer", Some("3.09.00")),
            ("MaxCharacterSlot", Some("20")),
            ("UserCareSystem", Some("2.00.14")),
            ("DisableInGameCheat", None),
            ("DisableCharacterSlotCheat", Some("1")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.map(str::to_owned))),
    )
    .unwrap();
    assert!(matches!(
        query_empty_admitted(&f, &pool, 0, &policy, now).await,
        Err(LobbyQueryError::Session(_))
    ));
    let admission = super::lab_admission();
    let name = "AdmittedLobby";
    let uid = vindictus_server::protocol::identity::derive_lab_identity(
        Some(name),
        admission.identity_options,
    )
    .unwrap()
    .nexon_sn;
    provision_game_user(&pool, uid, name, 1, 0).await.unwrap();
    let (frontend, token) = super::begin_lab(&registry, name, &admission);
    f = frontend;
    let r = evaluate_existing_lab_user(
        &mut f,
        token,
        &pool,
        0,
        PolicyClock {
            application: ApplicationClockSamples {
                set_lock_now: now,
                check_lock_now: now,
            },
            sql_local_now: now.sql_parameter().unwrap(),
        },
        10,
    )
    .await
    .unwrap();
    let empty = query_empty_admitted(&f, &pool, r.revision, &policy, now)
        .await
        .unwrap();
    assert_eq!(empty.max_free, 20);
    let body = vindictus_server::application::character_lobby::prepare_empty_character_list(
        &f, &pool, r.revision, &policy, now,
    )
    .await
    .unwrap();
    let message = vindictus_server::protocol::empty_character_list::decode(&body, 100)
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            message.characters_present,
            message.max_free,
            message.max_purchased,
            message.max_premium
        ),
        (true, 20, 0, 0)
    );
    assert!(message.party_states.unwrap().is_empty());
    let disabled = FeaturePolicy::new(
        [
            ("CurrentVer", Some("3.09.00")),
            ("MaxCharacterSlot", Some("20")),
            ("UserCareSystem", None),
            ("DisableInGameCheat", None),
            ("DisableCharacterSlotCheat", Some("1")),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.map(str::to_owned))),
    )
    .unwrap();
    query_empty_admitted(
        &f,
        &pool,
        r.revision,
        &disabled,
        SqlDateTime2::from_ticks(0).unwrap(),
    )
    .await
    .unwrap();

    let missing = FeaturePolicy::new([("CurrentVer".into(), Some("3.09.00".into()))]).unwrap();
    assert!(matches!(
        query_empty_admitted(&f, &pool, r.revision, &missing, now).await,
        Err(LobbyQueryError::Profile)
    ));
    f.close();
    assert!(matches!(
        query_empty_admitted(&f, &pool, r.revision, &policy, now).await,
        Err(LobbyQueryError::Session(_))
    ));
    storage::check_ready(&pool).await.unwrap();
    sqlx::query("ALTER TABLE game_user_info DROP COLUMN prologue_played")
        .execute(&pool)
        .await
        .unwrap();
    assert!(storage::check_ready(&pool).await.is_err());
    cleanup_game_pool(admin, pool, schema).await;
}
