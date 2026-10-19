use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use sqlx::PgPool;
use vindictus_server::{
    error::Error,
    infrastructure::{character_lobby::UserKey, postgres, user_captcha as db},
};
fn input() -> Value {
    json!({"format_version":1,"source":"study_heroes.user-captcha","rows":[{"uid":1,"punished_count":null,"failure_count":255,"last_failure_units300":"-1391644800000","last_succeed_units300":"76683386879999","condition":255},{"uid":-7,"punished_count":0,"failure_count":null,"last_failure_units300":"-1","last_succeed_units300":null,"condition":null}]})
}
fn snapshot(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
async fn roots(p: &PgPool) {
    sqlx::query("INSERT INTO game_users(id,name,next_character_sn)VALUES(1,'captcha_a',0),(2,'captcha_b',0)").execute(p).await.unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_preservation_full_rows_nullable_orphan_concurrent_replay_and_tamper() {
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let v = input();
    let snap = snapshot(&v);
    let (x, y) = tokio::join!(
        db::preserve(&p, &snap, false),
        db::preserve(&p, &snap, false)
    );
    assert_eq!(x.unwrap().inserted + y.unwrap().inserted, 2);
    type R = (
        i32,
        Option<i16>,
        Option<i16>,
        Option<i64>,
        Option<i64>,
        Option<i16>,
    );
    let actual: Vec<R> = sqlx::query_as("SELECT * FROM game_user_captcha ORDER BY uid")
        .fetch_all(&p)
        .await
        .unwrap();
    assert_eq!(
        actual,
        vec![
            (-7, Some(0), None, Some(-1), None, None),
            (
                1,
                None,
                Some(255),
                Some(-1391644800000),
                Some(76683386879999),
                Some(255)
            )
        ]
    );
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 0);
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 2);
    let mut changed = v;
    changed["rows"][0]["condition"] = json!(1);
    assert!(matches!(
        db::preserve(&p, &snapshot(&changed), false).await,
        Err(db::CaptchaError::Conflict)
    ));
    for assignment in [
        "punished_count=1",
        "failure_count=NULL",
        "last_failure_units300=0",
        "last_succeed_units300=NULL",
        "condition=1",
        "uid=10",
    ] {
        let mut tx = p.begin().await.unwrap();
        sqlx::query(&format!(
            "UPDATE game_user_captcha SET {assignment} WHERE uid=1"
        ))
        .execute(&mut *tx)
        .await
        .unwrap();
        tx.commit().await.unwrap();
        assert!(matches!(
            db::preserve(&p, &snap, true).await,
            Err(db::CaptchaError::Conflict)
        ));
        sqlx::raw_sql("DELETE FROM game_user_captcha;DELETE FROM game_user_captcha_preservation")
            .execute(&p)
            .await
            .unwrap();
        db::preserve(&p, &snap, false).await.unwrap();
    }
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_bootstrap_verify_missing_source_conflict_and_late_failure_are_atomic() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot(&input());
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::CaptchaError::Conflict)
    ));
    sqlx::query("INSERT INTO game_user_captcha(uid)VALUES(99)")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::CaptchaError::Conflict)
    ));
    sqlx::raw_sql("DELETE FROM game_user_captcha;CREATE FUNCTION reject_captcha_late() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.uid=1 THEN RAISE EXCEPTION 'late failure';END IF;RETURN NEW;END$$;CREATE TRIGGER reject_captcha_late BEFORE INSERT ON game_user_captcha FOR EACH ROW EXECUTE FUNCTION reject_captcha_late()").execute(&p).await.unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::CaptchaError::Database(_))
    ));
    for table in ["game_user_captcha", "game_user_captcha_preservation"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 0);
    }
    sqlx::query("DROP TRIGGER reject_captcha_late ON game_user_captcha")
        .execute(&p)
        .await
        .unwrap();
    db::preserve(&p, &snap, false).await.unwrap();
    sqlx::query("INSERT INTO game_user_captcha(uid)VALUES(99)")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::CaptchaError::Conflict)
    ));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_absence_requires_initialized_current_owner_blocks_insert_and_never_writes() {
    let (a, p, s) = isolated_game_pool().await;
    roots(&p).await;
    let owner = UserKey {
        uid: 2,
        name: "captcha_b",
        revision: 0,
    };
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, &owner).await,
        Err(db::CaptchaError::Uninitialized)
    ));
    tx.rollback().await.unwrap();
    db::preserve(&p, &snapshot(&input()), false).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_captcha_write() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected write';END$$").execute(&p).await.unwrap();
    for table in ["game_user_captcha", "game_user_captcha_preservation"] {
        sqlx::query(&format!("CREATE TRIGGER reject_captcha_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_captcha_write()")).execute(&p).await.unwrap();
    }
    let mut tx = p.begin().await.unwrap();
    db::require_absent(&mut tx, &owner).await.unwrap();
    for bad in [
        UserKey {
            uid: 1,
            name: "captcha_a",
            revision: 0,
        },
        UserKey {
            uid: 99,
            name: "missing",
            revision: 0,
        },
        UserKey {
            uid: 2,
            name: "wrong",
            revision: 0,
        },
        UserKey {
            uid: 2,
            name: "captcha_b",
            revision: 1,
        },
    ] {
        let result = db::require_absent(&mut tx, &bad).await;
        if bad.uid == 1 {
            assert!(matches!(result, Err(db::CaptchaError::Existing)))
        } else {
            assert!(matches!(result, Err(db::CaptchaError::Owner)))
        }
    }
    let mut other = p.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *other)
        .await
        .unwrap();
    let error = sqlx::query("INSERT INTO game_user_captcha(uid)VALUES(2)")
        .execute(&mut *other)
        .await
        .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03"),
        "must fail on the held table lock before reaching the write-rejection trigger"
    );
    other.rollback().await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        db::preserve(&p, &snapshot(&input()), true)
            .await
            .unwrap()
            .verified,
        2
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_readiness_migration_replay_constraints_and_reference_stability() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot(&input());
    db::preserve(&p, &snap, false).await.unwrap();
    postgres::migrate(&p).await.unwrap();
    postgres::check_ready(&p).await.unwrap();
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().inserted, 0);
    for assignment in [
        "punished_count=-1",
        "failure_count=256",
        "condition=256",
        "last_failure_units300=-1391644800001",
        "last_succeed_units300=76683386880000",
    ] {
        assert!(
            sqlx::query(&format!(
                "UPDATE game_user_captcha SET {assignment} WHERE uid=1"
            ))
            .execute(&p)
            .await
            .is_err()
        );
    }
    for table in ["game_user_captcha", "game_user_captcha_preservation"] {
        let column = if table == "game_user_captcha" {
            "last_succeed_units300"
        } else {
            "source_sha256"
        };
        sqlx::query(&format!(
            "ALTER TABLE {table} RENAME COLUMN {column} TO missing"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            postgres::check_ready(&p).await,
            Err(Error::SchemaNotReady)
        ));
        sqlx::query(&format!(
            "ALTER TABLE {table} RENAME COLUMN missing TO {column}"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_schema28_backfills_only_preverified_schema27_source_set() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot(&input());
    db::preserve(&p, &snap, false).await.unwrap();
    sqlx::raw_sql("DROP TABLE game_quick_slots,game_expanded_slots,game_manufacture,game_manufacture_recipes,game_new_recipes,game_item_loader_preservation;DELETE FROM _sqlx_migrations WHERE version=31;DROP TABLE game_newbie_attendance_info,game_newbie_attendance_preservation;DELETE FROM _sqlx_migrations WHERE version=30;DROP TABLE game_character_transfer_info,game_character_transfer_guard;DELETE FROM _sqlx_migrations WHERE version=29;DROP TABLE game_user_captcha_native_state;ALTER TABLE game_user_captcha_preservation DROP COLUMN source_uids;DELETE FROM _sqlx_migrations WHERE version=28").execute(&p).await.unwrap();
    postgres::migrate(&p).await.unwrap();
    let ids: Vec<i32> = sqlx::query_scalar(
        "SELECT source_uids FROM game_user_captcha_preservation WHERE singleton",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(ids, vec![-7, 1]);
    let verified = db::preserve(&p, &snap, true).await.unwrap();
    assert_eq!((verified.verified, verified.native_verified), (2, 0));
    cleanup_game_pool(a, p, s).await;
}
