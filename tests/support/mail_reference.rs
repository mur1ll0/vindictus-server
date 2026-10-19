use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use vindictus_server::infrastructure::{mail_reference as db, postgres};
mod fixture {
    include!("../fixtures/mail_reference_support.rs");
}
fn snapshot() -> db::Snapshot {
    db::read_snapshot(serde_json::to_vec(&fixture::data()).unwrap().as_slice()).unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_all_fields_constraints_orphans_and_exact_replay_are_preserved() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot();
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 7);
    let row: Value = sqlx::query_scalar("SELECT to_jsonb(m) FROM game_mail m WHERE mail_id=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(
        row,
        json!({"mail_id":1,"from_cid":5,"from_name_utf16":"\\x00d8","to_cid":6,"to_name_utf16":"\\x2000","mail_type":255,"posted_units300":-25919999,"title_utf16":"\\x41000000","content_utf16":"\\x00dc","arrive_units300":2,"expire_units300":null,"is_read":null,"charged_gold":-2147483648_i32,"is_deleted":false})
    );
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 0);
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 7);
    for assignment in [
        "mail_id=99",
        "from_cid=99",
        "from_name_utf16=''::bytea",
        "to_cid=99",
        "to_name_utf16=''::bytea",
        "mail_type=0",
        "posted_units300=0",
        "title_utf16=''::bytea",
        "content_utf16=''::bytea",
        "arrive_units300=0",
        "expire_units300=0",
        "is_read=false",
        "charged_gold=0",
        "is_deleted=true",
    ] {
        sqlx::query(&format!(
            "UPDATE game_mail SET {assignment} WHERE mail_id=1"
        ))
        .execute(&p)
        .await
        .unwrap();
        assert!(matches!(
            db::preserve(&p, &snap, false).await,
            Err(db::MailError::Reference)
        ));
        sqlx::raw_sql("DELETE FROM game_mail;DELETE FROM game_mail_preservation")
            .execute(&p)
            .await
            .unwrap();
        db::preserve(&p, &snap, false).await.unwrap();
    }
    for assignment in [
        "mail_type=-1",
        "mail_type=256",
        "posted_units300=-1391644800001",
        "arrive_units300=76683386880000",
        "expire_units300=76683386880000",
        "from_name_utf16=decode('01','hex')",
        "to_name_utf16=decode(repeat('00',42),'hex')",
        "title_utf16=decode(repeat('00',514),'hex')",
        "content_utf16=decode(repeat('00',2050),'hex')",
    ] {
        assert!(
            sqlx::query(&format!(
                "UPDATE game_mail SET {assignment} WHERE mail_id=1"
            ))
            .execute(&p)
            .await
            .is_err()
        );
    }
    let identities:i64=sqlx::query_scalar("SELECT count(*) FROM pg_attrdef d JOIN pg_class t ON t.oid=d.adrelid WHERE t.relname='game_mail' AND pg_get_expr(d.adbin,d.adrelid)LIKE '%nextval%'").fetch_one(&p).await.unwrap();
    assert_eq!(identities, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_negative_query_matches_complete_original_sql_on_synthetic_cases() {
    let (a, p, s) = isolated_game_pool().await;
    db::preserve(&p, &snapshot(), false).await.unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_mail_dml()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected mail DML';END$$;CREATE TRIGGER reject_mail_dml BEFORE INSERT OR UPDATE OR DELETE ON game_mail FOR EACH STATEMENT EXECUTE FUNCTION reject_mail_dml()").execute(&p).await.unwrap();
    for c in fixture::oracle()["cases"].as_array().unwrap() {
        let mut tx = p.begin().await.unwrap();
        let out = db::require_query_empty(&mut tx, c["cid"].as_i64().unwrap()).await;
        if c["empty"].as_bool().unwrap() {
            out.unwrap()
        } else {
            assert!(matches!(out, Err(db::MailError::Present)))
        }
        tx.rollback().await.unwrap();
    }
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_missing_reference_lost_routes_and_new_native_mail_never_become_false_empty() {
    let (a, p, s) = isolated_game_pool().await;
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_query_empty(&mut tx, 0).await,
        Err(db::MailError::Reference)
    ));
    tx.rollback().await.unwrap();
    db::preserve(&p, &snapshot(), false).await.unwrap();
    sqlx::query("DELETE FROM game_mail WHERE mail_id IN(1,7)")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_query_empty(&mut tx, 5).await,
        Err(db::MailError::Reference)
    ));
    tx.rollback().await.unwrap();
    sqlx::query("INSERT INTO game_mail SELECT 99,100,from_name_utf16,101,to_name_utf16,mail_type,posted_units300,title_utf16,content_utf16,arrive_units300,expire_units300,is_read,charged_gold,true FROM game_mail WHERE mail_id=2").execute(&p).await.unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_query_empty(&mut tx, 100).await,
        Err(db::MailError::Present)
    ));
    db::require_query_empty(&mut tx, 101).await.unwrap();
    tx.rollback().await.unwrap();
    sqlx::query("UPDATE game_mail_preservation SET source_return_cids=ARRAY[5,4]::bigint[]")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_query_empty(&mut tx, 0).await,
        Err(db::MailError::Reference)
    ));
    tx.rollback().await.unwrap();
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_late_import_failure_rolls_back_and_parallel_bootstrap_elects_one_writer() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot();
    sqlx::raw_sql("CREATE FUNCTION reject_last_mail()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF NEW.mail_id=7 THEN RAISE EXCEPTION 'synthetic late failure';END IF;RETURN NEW;END$$;CREATE TRIGGER reject_last_mail BEFORE INSERT ON game_mail FOR EACH ROW EXECUTE FUNCTION reject_last_mail()").execute(&p).await.unwrap();
    assert!(db::preserve(&p, &snap, false).await.is_err());
    let count: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*)FROM game_mail)+(SELECT count(*)FROM game_mail_preservation)",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(count, 0);
    sqlx::query("DROP TRIGGER reject_last_mail ON game_mail")
        .execute(&p)
        .await
        .unwrap();
    let (x, y) = tokio::join!(
        db::preserve(&p, &snap, false),
        db::preserve(&p, &snap, false)
    );
    assert_eq!(x.unwrap().inserted + y.unwrap().inserted, 7);
    let mut bytes = serde_json::to_vec(&fixture::data()).unwrap();
    bytes.push(b' ');
    let other = db::read_snapshot(bytes.as_slice()).unwrap();
    assert!(matches!(
        db::preserve(&p, &other, false).await,
        Err(db::MailError::Reference)
    ));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_negative_lookup_retains_lock_and_schema_readiness_checks_all_columns() {
    let (a, p, s) = isolated_game_pool().await;
    db::preserve(&p, &snapshot(), false).await.unwrap();
    let mut guard = p.begin().await.unwrap();
    db::require_query_empty(&mut guard, 0).await.unwrap();
    let mut writer = p.begin().await.unwrap();
    sqlx::query("SET LOCAL lock_timeout='100ms'")
        .execute(&mut *writer)
        .await
        .unwrap();
    let e = sqlx::query("UPDATE game_mail SET from_cid=0 WHERE mail_id=1")
        .execute(&mut *writer)
        .await
        .unwrap_err();
    assert_eq!(
        e.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("55P03")
    );
    writer.rollback().await.unwrap();
    guard.rollback().await.unwrap();
    sqlx::query("UPDATE game_mail SET from_cid=0 WHERE mail_id=1")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::require_query_empty(&mut tx, 0).await,
        Err(db::MailError::Present)
    ));
    tx.rollback().await.unwrap();
    postgres::check_ready(&p).await.unwrap();
    sqlx::query("ALTER TABLE game_mail DROP COLUMN is_deleted")
        .execute(&p)
        .await
        .unwrap();
    assert!(postgres::check_ready(&p).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
