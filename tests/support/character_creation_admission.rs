use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::character_creation_admission::CreationSlots,
    infrastructure::{
        character_creation_admission::{self as admission, CreationAdmissionError as E},
        character_creation_core::{self as core, CreationCoreRequest, CreationCoreResult},
        character_lobby::UserKey,
        lab_clock,
    },
};
fn key() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
async fn setup(p: &sqlx::PgPool) {
    provision_game_user(p, 31, "Owner", 42, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31")
        .bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes) VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
}
async fn character(p: &sqlx::PgPool, id: i64, sn: i32) {
    assert_eq!(
        core::create_base(
            p,
            key(),
            &common::proof(&common::content()),
            CreationCoreRequest {
                cid: id,
                character_sn: sn,
                name: "Murael",
                class: 1,
                use_preset: false
            }
        )
        .await
        .unwrap(),
        CreationCoreResult::Created
    );
    // Source deleted rows still participate in CharacterMaker's initial-SN probe.
    sqlx::query("UPDATE game_character_info SET status=1 WHERE id=$1")
        .bind(id)
        .execute(p)
        .await
        .unwrap();
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_admission_counts_visible_parents_and_slot_rows_ignoring_expiry() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    for (id, sn) in [(1, 101), (2, 102), (3, 103), (4, 104)] {
        character(&pool, id, sn).await;
    }
    sqlx::query("UPDATE game_character_info SET status=CASE id WHEN 1 THEN 0 WHEN 2 THEN 2 WHEN 3 THEN 1 ELSE 3 END").execute(&pool).await.unwrap();
    for (id, owner, name, expires) in [
        (1, 31, "CharacterSlot", Some(0i64)),
        (2, 31, "characterslot  ", None),
        (3, 31, "PREMIUMCHARACTERSLOT", Some(0)),
        (4, 31, "Other", None),
        (5, 32, "CharacterSlot", None),
    ] {
        sqlx::query("INSERT INTO game_user_products(id,nexon_sn,product_name_utf16,argument_utf16,expire_time_ticks) VALUES($1,$2,$3,$4,$5)")
            .bind(id as i64).bind(owner).bind(utf16(name)).bind(utf16("Ignored")).bind(expires).execute(&pool).await.unwrap();
    }
    let result = admission::slots(&pool, key()).await.unwrap();
    assert_eq!(
        result,
        CreationSlots {
            visible: 2,
            purchased: 2,
            premium: 1
        }
    );
    assert!(!result.at_limit(0));
    assert!(result.at_limit(-1));
    assert!(matches!(
        admission::slots(
            &pool,
            UserKey {
                revision: 1,
                ..key()
            }
        )
        .await,
        Err(E::Ownership)
    ));
    sqlx::query("UPDATE game_user_products SET product_name_utf16=$1 WHERE id=4")
        .bind(utf16("\u{4e00}"))
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        admission::slots(&pool, key()).await,
        Err(E::Profile)
    ));
    sqlx::query("DELETE FROM game_user_products")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_user_products(nexon_sn,product_name_utf16) SELECT 31,$1 FROM generate_series(1,1001)").bind(utf16("Other")).execute(&pool).await.unwrap();
    assert!(matches!(
        admission::slots(&pool, key()).await,
        Err(E::Profile)
    ));
    sqlx::query("DELETE FROM game_lab_account_origins")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        admission::slots(&pool, key()).await,
        Err(E::Ownership)
    ));
    let logs:i64=sqlx::query_scalar("SELECT (SELECT COUNT(*) FROM game_character_ledger01)+(SELECT COUNT(*) FROM game_character_ledger02)").fetch_one(&pool).await.unwrap();
    assert_eq!(logs, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_serial_preserves_zero_retry_initial_probe_and_256_iteration_exit() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    let now = lab_clock::unix_ticks(0, 0).unwrap();
    let mut calls = 0;
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../fixtures/framework-character-creation-probe.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["profiles"][0]["cases"],
        fixture["profiles"][1]["cases"]
    );
    let cases = fixture["profiles"][0]["cases"].as_array().unwrap();
    let first = admission::serial_with_clock(&pool, key(), -1, || {
        calls += 1;
        Ok(now)
    })
    .await
    .unwrap();
    assert_eq!(i64::from(first.serial), cases[0]["sn"].as_i64().unwrap());
    assert_eq!(calls, cases[0]["clock_samples"].as_i64().unwrap()); // first=0, zero retry=256
    character(&pool, 1, 256).await;
    // A different latest-output SN does NOT stop the source's initial-SN loop.
    let mut calls = 0;
    let collided = admission::serial_with_clock(&pool, key(), -1, || {
        calls += 1;
        Ok(now)
    })
    .await
    .unwrap();
    assert_eq!(i64::from(collided.serial), cases[1]["sn"].as_i64().unwrap());
    assert_eq!(calls, cases[1]["clock_samples"].as_i64().unwrap());
    assert_eq!(cases[1]["queries"], 256);
    assert_eq!(cases[1]["keys"], serde_json::json!([256]));
    assert_eq!(
        collided.delay_ticks,
        i64::from(collided.serial) * 10_000_000
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_characters")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    // SingleOrDefault sees both deleted rows; SQL base's active-only guard is different.
    character(&pool, 2, 256).await;
    let mut calls = 0;
    assert!(matches!(
        admission::serial_with_clock(&pool, key(), -1, || {
            calls += 1;
            Ok(now)
        })
        .await,
        Err(E::MultipleSerialRows)
    ));
    assert_eq!(calls, cases[2]["clock_samples"].as_i64().unwrap());
    assert_eq!(cases[2]["error"], "InvalidOperationException");
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn creation_admission_cancellation_and_scope_refusal_leave_state_unchanged() {
    let (admin, pool, schema) = isolated_game_pool().await;
    setup(&pool).await;
    sqlx::query("DELETE FROM game_lab_account_origins")
        .execute(&pool)
        .await
        .unwrap();
    let mut calls = 0;
    assert!(matches!(
        admission::serial_with_clock(&pool, key(), 0, || {
            calls += 1;
            Ok(lab_clock::unix_ticks(0, 0).unwrap())
        })
        .await,
        Err(E::Ownership)
    ));
    assert_eq!(calls, 0);
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(&pool).await.unwrap();
    assert!(matches!(
        admission::serial_with_clock(&pool, key(), 0, || Err(E::Clock)).await,
        Err(E::Clock)
    ));
    let mut block = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_character_info IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *block)
        .await
        .unwrap();
    let p = pool.clone();
    let task = tokio::spawn(async move { admission::slots(&p, key()).await });
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    block.rollback().await.unwrap();
    assert_eq!(
        admission::slots(&pool, key()).await.unwrap(),
        CreationSlots {
            visible: 0,
            purchased: 0,
            premium: 0
        }
    );
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_characters")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    cleanup_game_pool(admin, pool, schema).await;
}
