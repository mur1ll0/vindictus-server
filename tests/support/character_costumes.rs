use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::character_costume::Costume,
    infrastructure::{
        character_costumes::{self as db, CostumeError},
        character_creation_core::{self as core, CreationCoreRequest},
        character_lobby::UserKey,
    },
};
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
fn export() -> Value {
    json!({"format_version":1,"source":"study_heroes.costume","source_last_value":"10","rows":[
{"row_id":"2","cid":"1","is_base":true,"costume_type":0,"costume_key":2,"costume_sn":null,"c1":-2147483648i64,"c2":0,"c3":2147483647,"gloss":255,"effect":-7},
{"row_id":"7","cid":"1","is_base":true,"costume_type":0,"costume_key":2,"costume_sn":-1,"c1":null,"c2":null,"c3":null,"gloss":null,"effect":null}]})
}
fn empty() -> db::Snapshot {
    snapshot(
        &json!({"format_version":1,"source":"study_heroes.costume","source_last_value":null,"rows":[]}),
    )
}
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
async fn count(p: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM game_costumes")
        .fetch_one(p)
        .await
        .unwrap()
}
async fn setup(p: &PgPool) {
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
    let proof = common::proof(&common::content());
    core::create_base(
        p,
        key(),
        &proof,
        CreationCoreRequest {
            cid: 1,
            character_sn: 12,
            name: "Murael",
            class: 1,
            use_preset: false,
        },
    )
    .await
    .unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn costume_preserves_all_fields_duplicate_keys_concurrent_import_and_identity_without_reseed()
{
    let (admin, p, schema) = isolated_game_pool().await;
    sqlx::query("INSERT INTO game_characters VALUES(1)")
        .execute(&p)
        .await
        .unwrap();
    let mut bad = export();
    bad["rows"][1]["cid"] = json!("99");
    assert!(db::preserve(&p, &snapshot(&bad), false).await.is_err());
    assert_eq!(count(&p).await, 0);
    let state: bool = sqlx::query_scalar("SELECT initialized FROM game_costume_identity")
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(!state);
    assert!(db::preserve(&p, &snapshot(&export()), true).await.is_err());
    let s = Arc::new(snapshot(&export()));
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..2 {
        let p = p.clone();
        let s = Arc::clone(&s);
        tasks.spawn(async move { db::preserve(&p, &s, false).await.unwrap().inserted });
    }
    let mut inserted = 0;
    while let Some(r) = tasks.join_next().await {
        inserted += r.unwrap()
    }
    assert_eq!(inserted, 2);
    assert_eq!(count(&p).await, 2);
    assert!(
        db::preserve(&p, &s, true)
            .await
            .unwrap()
            .identity_initialized
    );
    for column in [
        "cid",
        "is_base",
        "costume_type",
        "costume_key",
        "costume_sn",
        "c1",
        "c2",
        "c3",
        "gloss",
        "effect",
    ] {
        let mut conflict = export();
        conflict["rows"][1][column] = match column {
            "cid" => json!("2"),
            "is_base" => json!(false),
            _ => json!(13),
        };
        assert!(
            matches!(
                db::preserve(&p, &snapshot(&conflict), false).await,
                Err(CostumeError::Conflict)
            ),
            "{column}"
        );
    }
    let first: i64 = sqlx::query_scalar("SELECT nextval('game_costume_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(first, 11);
    assert_eq!(db::preserve(&p, &s, false).await.unwrap().inserted, 0);
    let next: i64 = sqlx::query_scalar("SELECT nextval('game_costume_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(next, 12);
    let mut changed = export();
    changed["source_last_value"] = json!("11");
    assert!(db::preserve(&p, &snapshot(&changed), false).await.is_err());
    sqlx::query("UPDATE game_characters SET id=2 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    let owners: Vec<i64> = sqlx::query_scalar("SELECT cid FROM game_costumes")
        .fetch_all(&p)
        .await
        .unwrap();
    assert_eq!(owners, vec![2, 2]);
    sqlx::query("DELETE FROM game_characters WHERE id=2")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(count(&p).await, 0);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn costume_save_requires_owner_revision_origin_and_preserves_alias_effect_and_nulls() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        db::save(&p, &key(), 1, &[]).await,
        Err(CostumeError::Uninitialized)
    ));
    db::preserve(&p, &empty(), false).await.unwrap();
    let c = Costume {
        effect: Some(91),
        colors: [Some(-1), Some(0), None],
        gloss: Some(255),
        ..Costume::extra(2, 42)
    };
    let newer = Costume {
        serial: None,
        colors: [None, Some(i32::MIN), Some(i32::MAX)],
        gloss: None,
        effect: Some(19),
        ..c
    };
    assert_eq!(db::save(&p, &key(), 1, &[c, newer]).await.unwrap(), 1);
    type SavedAppearance = (
        i64,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<i32>,
        Option<i16>,
        Option<i32>,
    );
    let actual: SavedAppearance =
        sqlx::query_as("SELECT row_id,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(
        actual,
        (
            1,
            None,
            None,
            Some(i32::MIN),
            Some(i32::MAX),
            None,
            Some(91)
        )
    );
    assert_eq!(db::save(&p, &key(), 1, &[newer]).await.unwrap(), 0);
    db::save(
        &p,
        &key(),
        1,
        &[
            Costume::extra(8, 8),
            Costume::extra(7, 7),
            Costume::extra(6, 6),
            Costume::extra(8, 80),
        ],
    )
    .await
    .unwrap();
    let ordered:Vec<(i64,i16,Option<i32>)>=sqlx::query_as("SELECT row_id,costume_key,costume_sn FROM game_costumes WHERE costume_key>=6 ORDER BY row_id").fetch_all(&p).await.unwrap();
    assert_eq!(
        ordered,
        vec![(2, 8, Some(80)), (3, 7, Some(7)), (4, 6, Some(6))]
    );
    let stale = UserKey {
        revision: 1,
        ..key()
    };
    assert!(matches!(
        db::save(&p, &stale, 1, &[c]).await,
        Err(CostumeError::Ownership)
    ));
    assert!(db::save(&p, &key(), 99, &[c]).await.is_err());
    sqlx::query("DELETE FROM game_lab_account_origins")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::save(&p, &key(), 1, &[c]).await,
        Err(CostumeError::Ownership)
    ));
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&p).await.unwrap();
    sqlx::query("INSERT INTO game_costumes SELECT 99,cid,is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes WHERE row_id=1").execute(&p).await.unwrap();
    assert!(matches!(
        db::save(&p, &key(), 1, &[c]).await,
        Err(CostumeError::Profile(_))
    ));
    assert_eq!(count(&p).await, 5);
    sqlx::query("DELETE FROM game_characters WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(count(&p).await, 0);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn costume_late_failure_rolls_back_updates_and_rows_but_consumes_identity() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    db::preserve(&p, &empty(), false).await.unwrap();
    db::save(&p, &key(), 1, &[Costume::extra(0, 42)])
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_costume() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.costume_key=2 THEN RAISE EXCEPTION 'synthetic late failure'; END IF; RETURN NEW; END $$; CREATE TRIGGER costume_fail BEFORE INSERT ON game_costumes FOR EACH ROW EXECUTE FUNCTION reject_costume();").execute(&p).await.unwrap();
    assert!(
        db::save(
            &p,
            &key(),
            1,
            &[
                Costume::extra(0, 99),
                Costume::extra(1, 1),
                Costume::extra(2, 2)
            ]
        )
        .await
        .is_err()
    );
    assert_eq!(count(&p).await, 1);
    let sn: i32 = sqlx::query_scalar("SELECT costume_sn FROM game_costumes")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(sn, 42);
    let last: i64 = sqlx::query_scalar("SELECT last_value FROM game_costume_row_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(last, 3);
    db::save(&p, &key(), 1, &[Costume::extra(1, 1)])
        .await
        .unwrap();
    let id: i64 = sqlx::query_scalar("SELECT row_id FROM game_costumes WHERE costume_key=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(id, 4);
    cleanup_game_pool(admin, p, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn costume_cancellation_rolls_back_inflight_writes_and_keeps_identity_gap() {
    let (admin, p, schema) = isolated_game_pool().await;
    setup(&p).await;
    db::preserve(&p, &empty(), false).await.unwrap();
    let lock_id = uuid::Uuid::new_v4().as_u128() as i64;
    let sql = format!(
        "CREATE FUNCTION block_costume() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.costume_key=2 THEN PERFORM pg_advisory_xact_lock({lock_id}); END IF; RETURN NEW; END $$; CREATE TRIGGER costume_block BEFORE INSERT ON game_costumes FOR EACH ROW EXECUTE FUNCTION block_costume();"
    );
    sqlx::raw_sql(&sql).execute(&p).await.unwrap();
    let mut block = p.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_id)
        .execute(&mut *block)
        .await
        .unwrap();
    let cp = p.clone();
    let task = tokio::spawn(async move {
        db::save(
            &cp,
            &key(),
            1,
            &[Costume::extra(1, 1), Costume::extra(2, 2)],
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2),async{loop{let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND NOT granted AND database=(SELECT oid FROM pg_database WHERE datname=current_database()))").fetch_one(&p).await.unwrap();if waiting{break}tokio::time::sleep(Duration::from_millis(10)).await}}).await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    assert_eq!(count(&p).await, 0);
    let last: i64 = sqlx::query_scalar("SELECT last_value FROM game_costume_row_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(last, 2);
    cleanup_game_pool(admin, p, schema).await;
}
