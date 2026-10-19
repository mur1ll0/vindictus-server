use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use vindictus_server::infrastructure::{newbie_attendance as db, postgres};
fn data() -> Value {
    json!({"format_version":1,"source":"study_heroes.newbie-attendance","rows":[{"uid":1,"event_version":7,"start_ticks":"0","end_ticks":"3155378975999999999"},{"uid":-7,"event_version":-1,"start_ticks":"17","end_ticks":"13"}]})
}
fn source(v: &Value) -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn newbie_attendance_full_source_replay_constraints_and_no_repair_on_field_drift() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = source(&data());
    let (x, y) = tokio::join!(
        db::preserve(&p, &snap, false),
        db::preserve(&p, &snap, false)
    );
    assert_eq!(x.unwrap().inserted + y.unwrap().inserted, 2);
    let actual: Vec<(i32, i32, i64, i64)> =
        sqlx::query_as("SELECT * FROM game_newbie_attendance_info ORDER BY uid")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(
        actual,
        vec![(-7, -1, 17, 13), (1, 7, 0, 3155378975999999999)]
    );
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 2);
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 0);
    for assignment in ["uid=42", "event_version=99", "start_ticks=1", "end_ticks=1"] {
        sqlx::query(&format!(
            "UPDATE game_newbie_attendance_info SET {assignment} WHERE uid=1"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            db::preserve(&p, &snap, false).await,
            Err(db::AttendanceError::Reference)
        ));
        sqlx::raw_sql("DELETE FROM game_newbie_attendance_info;DELETE FROM game_newbie_attendance_preservation").execute(&p).await.unwrap();
        db::preserve(&p, &snap, false).await.unwrap();
    }
    for assignment in ["start_ticks=-1", "end_ticks=3155378976000000000"] {
        assert!(
            sqlx::query(&format!(
                "UPDATE game_newbie_attendance_info SET {assignment} WHERE uid=1"
            ))
            .execute(&p)
            .await
            .is_err()
        );
    }
    postgres::migrate(&p).await.unwrap();
    postgres::check_ready(&p).await.unwrap();
    sqlx::query("ALTER TABLE game_newbie_attendance_info DROP COLUMN event_version")
        .execute(&p)
        .await
        .unwrap();
    assert!(postgres::check_ready(&p).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn newbie_attendance_absence_requires_initialized_uid_set_and_blocks_late_insertion() {
    let (a, p, s) = isolated_game_pool().await;
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, 31).await,
        Err(db::AttendanceError::Reference)
    ));
    tx.rollback().await.unwrap();
    let snap = source(&data());
    db::preserve(&p, &snap, false).await.unwrap();
    let mut held = p.begin().await.unwrap();
    db::require_absent(&mut held, 31).await.unwrap();
    assert!(matches!(
        db::require_absent(&mut held, 1).await,
        Err(db::AttendanceError::Present)
    ));
    let mut writer = p.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *writer)
        .await
        .unwrap();
    let err = sqlx::query("INSERT INTO game_newbie_attendance_info VALUES(31,7,0,0)")
        .execute(&mut *writer)
        .await
        .unwrap_err();
    assert_eq!(
        err.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    writer.rollback().await.unwrap();
    held.commit().await.unwrap();
    sqlx::query("DELETE FROM game_newbie_attendance_info WHERE uid=1")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_absent(&mut tx, 1).await,
        Err(db::AttendanceError::Reference)
    ));
    tx.rollback().await.unwrap();
    assert!(db::preserve(&p, &snap, true).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn newbie_attendance_bootstrap_refuses_unexplained_rows_and_rolls_back_late_failure() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = source(&data());
    assert!(db::preserve(&p, &snap, true).await.is_err());
    sqlx::query("INSERT INTO game_newbie_attendance_info VALUES(99,0,0,0)")
        .execute(&p)
        .await
        .unwrap();
    assert!(db::preserve(&p, &snap, false).await.is_err());
    sqlx::query("DELETE FROM game_newbie_attendance_info")
        .execute(&p)
        .await
        .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_attendance_import() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.uid=1 THEN RAISE EXCEPTION 'synthetic late import';END IF;RETURN NEW;END$$;CREATE TRIGGER reject_attendance_import BEFORE INSERT ON game_newbie_attendance_info FOR EACH ROW EXECUTE FUNCTION reject_attendance_import()").execute(&p).await.unwrap();
    assert!(db::preserve(&p, &snap, false).await.is_err());
    let n:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM game_newbie_attendance_info)+(SELECT count(*) FROM game_newbie_attendance_preservation)").fetch_one(&p).await.unwrap();
    assert_eq!(n, 0);
    cleanup_game_pool(a, p, s).await;
}
