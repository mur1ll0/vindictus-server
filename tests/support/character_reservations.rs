use super::{cleanup_game_pool, isolated_game_pool};
use vindictus_server::{
    application::game_users::provision_game_user,
    infrastructure::{
        character_lobby::UserKey,
        character_reservations::{self as storage, ReservationError},
    },
};
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn reservation_query_matches_source_sql_membership_without_consuming_or_uid_filter() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/sql-reservation-query.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        sqlx::query("DELETE FROM game_users")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM game_reserved_names")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM game_reserved_titles")
            .execute(&pool)
            .await
            .unwrap();
        let who = case["who"].as_str().unwrap();
        let mask = case["mask"].as_i64().unwrap() as i32;
        provision_game_user(&pool, 31, who, 1, 0).await.unwrap();
        let mut setup = pool.begin().await.unwrap();
        for (id, owner, uid, name, created) in [
            (1, "Owner".to_owned(), -51, "LabFiona", mask & 1 != 0),
            (2, "OWNER ".to_owned(), 777, "Éλ", mask & 2 != 0),
            (3, "Other".to_owned(), 31, "OtherChar", false),
            (4, "owner".to_owned(), -91, "LabFiona", mask & 1 != 0),
            (5, String::new(), 1, "EmptyOwner", false),
            (6, "   ".to_owned(), 2, "SpaceOwner", false),
            (7, "Z".repeat(50), 3, "LongOwner", false),
        ] {
            sqlx::query("INSERT INTO game_reserved_names VALUES($1,$2,$3,$4,$5)")
                .bind(id)
                .bind(utf16(&owner))
                .bind(uid)
                .bind(utf16(name))
                .bind(created)
                .execute(&mut *setup)
                .await
                .unwrap();
        }
        for (id, owner, uid, title, given) in [
            (1, "Owner".to_owned(), -51, i32::MIN, mask & 4 != 0),
            (2, "OWNER ".to_owned(), 777, i32::MAX, mask & 8 != 0),
            (3, "Other".to_owned(), 31, 42, false),
            (4, String::new(), 1, -1, false),
            (5, "   ".to_owned(), 2, 0, false),
            (6, "Z".repeat(50), 3, 17, false),
        ] {
            sqlx::query("INSERT INTO game_reserved_titles VALUES($1,$2,$3,$4,$5)")
                .bind(id)
                .bind(utf16(&owner))
                .bind(uid)
                .bind(title)
                .bind(given)
                .execute(&mut *setup)
                .await
                .unwrap();
        }
        setup.commit().await.unwrap();
        let before:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_reserved_names WHERE created),(SELECT COUNT(*) FROM game_reserved_titles WHERE title_given)").fetch_one(&pool).await.unwrap();
        let actual = storage::query(
            &pool,
            UserKey {
                uid: 31,
                name: who,
                revision: 0,
            },
        )
        .await
        .unwrap();
        let mut names: Vec<Vec<u8>> = actual.names.iter().map(|n| utf16(n)).collect();
        names.sort();
        let mut expected: Vec<Vec<u8>> = case["names"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| {
                n["utf16_hex"]
                    .as_str()
                    .unwrap()
                    .as_bytes()
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
                    .collect()
            })
            .collect();
        expected.sort();
        assert_eq!(names, expected);
        let mut titles = actual.titles;
        titles.sort();
        let mut expected: Vec<i32> = case["titles"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n["title_id"].as_i64().unwrap() as i32)
            .collect();
        expected.sort();
        assert_eq!(titles, expected);
        let after:(i64,i64)=sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_reserved_names WHERE created),(SELECT COUNT(*) FROM game_reserved_titles WHERE title_given)").fetch_one(&pool).await.unwrap();
        assert_eq!(before, after);
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn reservation_query_refuses_wrong_owner_stale_revision_unknown_collation_and_cancelled_lock()
{
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 31, "Owner", 1, 0).await.unwrap();
    for (uid, name, revision) in [(31, "Other", 0), (32, "Owner", 0), (31, "Owner", 1)] {
        assert!(matches!(
            storage::query(
                &pool,
                UserKey {
                    uid,
                    name,
                    revision
                }
            )
            .await,
            Err(ReservationError::Ownership)
        ));
    }
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM game_users WHERE id=31 FOR UPDATE")
        .execute(&mut *lock)
        .await
        .unwrap();
    assert!(
        tokio::time::timeout(
            std::time::Duration::from_millis(75),
            storage::query(
                &pool,
                UserKey {
                    uid: 31,
                    name: "Owner",
                    revision: 0
                }
            )
        )
        .await
        .is_err()
    );
    lock.rollback().await.unwrap();
    sqlx::query("INSERT INTO game_reserved_names VALUES(1,$1,31,$2,false)")
        .bind(utf16("Éowner"))
        .bind(utf16("SomeName"))
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        storage::query(
            &pool,
            UserKey {
                uid: 31,
                name: "Owner",
                revision: 0
            }
        )
        .await,
        Err(ReservationError::Profile)
    ));
    assert!(
        !sqlx::query_scalar::<_, bool>("SELECT created FROM game_reserved_names WHERE row_id=1")
            .fetch_one(&pool)
            .await
            .unwrap()
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn reservation_schema_preserves_utf16_bounds_keys_default_utc_and_empty_identity() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let state: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_reserved_names_row_id_seq")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, (1, false));
    let raw = vec![0, 0, 0, 216, 0, 220];
    sqlx::query("INSERT INTO game_reserved_names VALUES(-1,$1,-2147483648,$1,false)")
        .bind(&raw)
        .execute(&pool)
        .await
        .unwrap();
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT character_name_utf16 FROM game_reserved_names WHERE row_id=-1")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, raw);
    let state: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_reserved_names_row_id_seq")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(state, (1, false));
    let id:i32=sqlx::query_scalar("INSERT INTO game_reserved_names(nexon_id_utf16,nexon_sn,character_name_utf16,created) VALUES($1,31,$1,false) RETURNING row_id").bind(utf16("Owner")).fetch_one(&pool).await.unwrap();
    assert_eq!(id, 1);
    for bytes in [vec![0u8; 101], vec![0u8; 102]] {
        assert!(
            sqlx::query("INSERT INTO game_reserved_titles VALUES(1,$1,31,42,false)")
                .bind(bytes)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(sqlx::query("INSERT INTO game_reserved_titles(nexon_id_utf16,nexon_sn,title_id,title_given) VALUES($1,31,42,false)").bind(utf16("Owner")).execute(&pool).await.is_err());
    let mut conn = pool.acquire().await.unwrap();
    sqlx::query("SET TIME ZONE 'Pacific/Auckland'")
        .execute(&mut *conn)
        .await
        .unwrap();
    let before: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::bigint",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    let date:i64=sqlx::query_scalar("INSERT INTO game_character_name_logs(id,old_character_name_utf16,new_character_name_utf16) VALUES(77,$1,$2) RETURNING log_date_units300").bind(&raw).bind(utf16("NewName")).fetch_one(&mut *conn).await.unwrap();
    let after: i64 = sqlx::query_scalar(
        "SELECT floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::bigint",
    )
    .fetch_one(&mut *conn)
    .await
    .unwrap();
    assert!((before..=after).contains(&date));
    assert!(
        sqlx::query("INSERT INTO game_character_name_logs VALUES(77,$1,$1,$2)")
            .bind(&raw)
            .bind(date)
            .execute(&mut *conn)
            .await
            .is_err()
    );
    let fks:i64=sqlx::query_scalar("SELECT COUNT(*) FROM pg_constraint WHERE contype='f' AND conrelid IN('game_reserved_names'::regclass,'game_reserved_titles'::regclass,'game_character_name_logs'::regclass)").fetch_one(&mut *conn).await.unwrap();
    assert_eq!(fks, 0);
    drop(conn);
    cleanup_game_pool(admin, pool, schema).await;
}
