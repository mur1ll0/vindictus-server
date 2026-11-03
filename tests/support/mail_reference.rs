use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::{Value, json};
use vindictus_server::infrastructure::{mail_reference as db, minimum_mail, postgres};
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
    // What the mail lifecycle may change in a preserved row: IsRead and IsDeleted set.
    sqlx::query("UPDATE game_mail SET is_read=true,is_deleted=true WHERE mail_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 7);
    let identities:i64=sqlx::query_scalar("SELECT count(*) FROM pg_attrdef d JOIN pg_class t ON t.oid=d.adrelid WHERE t.relname='game_mail' AND pg_get_expr(d.adbin,d.adrelid)LIKE '%nextval%'").fetch_one(&p).await.unwrap();
    assert_eq!(identities, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_query_list_matches_complete_original_sql_on_synthetic_cases() {
    let (a, p, s) = isolated_game_pool().await;
    db::preserve(&p, &snapshot(), false).await.unwrap();
    // QueryMailList of the lifecycle against the original SELECT's rows of each CID.
    for c in fixture::oracle()["cases"].as_array().unwrap() {
        let mut ours = minimum_mail::query_mail_ids(&p, c["cid"].as_i64().unwrap())
            .await
            .unwrap();
        ours.sort_unstable();
        let mut theirs: Vec<i64> = c["mail_ids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        theirs.sort_unstable();
        assert_eq!(ours, theirs, "cid {}", c["cid"]);
        assert_eq!(ours.is_empty(), c["empty"].as_bool().unwrap());
    }
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn mail_verify_accepts_the_lifecycle_and_refuses_unjournaled_loss() {
    let (a, p, s) = isolated_game_pool().await;
    let snap = snapshot();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::MailError::Reference)
    ));
    db::preserve(&p, &snap, false).await.unwrap();
    // New native mail is not part of the reference.
    sqlx::query("INSERT INTO game_mail SELECT 99,100,from_name_utf16,101,to_name_utf16,mail_type,posted_units300,title_utf16,content_utf16,arrive_units300,expire_units300,is_read,charged_gold,true FROM game_mail WHERE mail_id=2").execute(&p).await.unwrap();
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 7);
    // A preserved row removed without its postal journal is a loss.
    sqlx::query("DELETE FROM game_mail WHERE mail_id=7")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::MailError::Reference)
    ));
    // Cleansed or returned by the lifecycle: the journal of that MailID accounts for it.
    sqlx::query("INSERT INTO game_postal_journals(mail_id,from_cid,to_cid) VALUES(7,0,0)")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(db::preserve(&p, &snap, true).await.unwrap().verified, 7);
    // IsRead and IsDeleted never go back.
    sqlx::query("UPDATE game_mail SET is_read=false WHERE mail_id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::MailError::Reference)
    ));
    sqlx::query("UPDATE game_mail SET is_read=NULL WHERE mail_id=1")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("UPDATE game_mail_preservation SET source_return_cids=ARRAY[5,4]::bigint[]")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, true).await,
        Err(db::MailError::Reference)
    ));
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
async fn mail_schema_readiness_checks_all_columns() {
    let (a, p, s) = isolated_game_pool().await;
    db::preserve(&p, &snapshot(), false).await.unwrap();
    postgres::check_ready(&p).await.unwrap();
    sqlx::query("ALTER TABLE game_mail DROP COLUMN is_deleted")
        .execute(&p)
        .await
        .unwrap();
    assert!(postgres::check_ready(&p).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
