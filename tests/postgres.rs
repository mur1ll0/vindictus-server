use argon2::{Argon2, PasswordHash, PasswordVerifier};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use vindictus_server::{
    application::accounts::create_account, domain::Username, error::Error, infrastructure::postgres,
};
use zeroize::Zeroizing;

/// Run against a disposable database: TEST_DATABASE_URL must permit CREATE SCHEMA.
/// Each run uses an isolated random schema and removes it on success.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn postgres_migration_accounts_and_schema_readiness() {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect PostgreSQL");
    let schema = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .expect("create isolated schema");
    let search_path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _| {
            let statement = search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .expect("isolated pool");
    assert!(
        postgres::check_ready(&pool).await.is_err(),
        "fresh schema cannot be ready"
    );
    postgres::migrate(&pool).await.expect("migration");
    postgres::migrate(&pool)
        .await
        .expect("idempotent migration");
    postgres::check_ready(&pool)
        .await
        .expect("ready after migration");
    let password = b"integration test password";
    let id = create_account(
        &pool,
        Username::parse("Study_User").expect("name"),
        Zeroizing::new(password.to_vec()),
    )
    .await
    .expect("account");
    let (normalized, stored): (String, String) =
        sqlx::query_as("SELECT normalized_name, password_hash FROM accounts WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("stored row");
    assert_eq!(normalized, "study_user");
    assert!(
        Argon2::default()
            .verify_password(password, &PasswordHash::new(&stored).expect("hash"))
            .is_ok()
    );
    assert!(matches!(
        create_account(
            &pool,
            Username::parse("STUDY_USER").expect("name"),
            Zeroizing::new(password.to_vec())
        )
        .await,
        Err(Error::AccountExists)
    ));
    sqlx::query("UPDATE _sqlx_migrations SET checksum = '\\x00'::bytea")
        .execute(&pool)
        .await
        .expect("simulate incompatible schema");
    assert!(matches!(
        postgres::check_ready(&pool).await,
        Err(Error::SchemaNotReady)
    ));
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .expect("cleanup schema");
    admin.close().await;
}

async fn isolated_game_pool() -> (sqlx::PgPool, sqlx::PgPool, String) {
    let url = std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required");
    let admin = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .after_connect(move |connection, _| {
            let statement = search_path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    postgres::migrate(&pool).await.unwrap();
    (admin, pool, schema)
}

async fn cleanup_game_pool(admin: sqlx::PgPool, pool: sqlx::PgPool, schema: String) {
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn game_users_preserve_source_values_reject_collisions_and_enforce_schema() {
    use vindictus_server::application::game_users::provision_game_user;
    use vindictus_server::protocol::identity::{
        AsciiCulture, FrameworkArchitecture, IdentityProfile, LabIdentityOptions,
        derive_lab_identity,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let created = provision_game_user(&pool, i32::MIN, "MiXeD ", -1, 255)
        .await
        .unwrap();
    assert!(created.created);
    assert_eq!(created.user.id, i32::MIN);
    assert_eq!(created.user.name, "MiXeD ");
    assert_eq!(created.user.next_character_sn, 0);
    assert_eq!(created.user.service_id, -1);
    assert_eq!(created.user.channeling_code, Some(255));
    sqlx::query("UPDATE game_users SET next_character_sn=7 WHERE id=$1")
        .bind(i32::MIN)
        .execute(&pool)
        .await
        .unwrap();
    let existing = provision_game_user(&pool, i32::MIN, "mixed", 99, 0)
        .await
        .unwrap();
    assert!(!existing.created);
    assert_eq!(existing.user.name, "MiXeD ");
    assert_eq!(existing.user.next_character_sn, 7);
    assert_eq!(existing.user.service_id, -1);
    assert_eq!(existing.user.channeling_code, Some(255));
    assert!(matches!(
        provision_game_user(&pool, i32::MAX, "MIXED  ", 99, 1).await,
        Err(Error::GameUserNameConflict)
    ));
    assert!(matches!(
        provision_game_user(&pool, i32::MIN, "another", 99, 1).await,
        Err(Error::GameUserIdConflict)
    ));
    let other = provision_game_user(&pool, i32::MAX, "another", 42, 0)
        .await
        .unwrap();
    assert!(other.user.channeling_code.is_none());
    // Both keys collide with two different rows: never return either user's data.
    assert!(matches!(
        provision_game_user(&pool, i32::MIN, "another", 99, 1).await,
        Err(Error::GameUserIdConflict)
    ));
    for id in [-1, 0, 1] {
        let name = format!("signed_{id}");
        assert_eq!(
            provision_game_user(&pool, id, &name, 42, 0)
                .await
                .unwrap()
                .user
                .id,
            id
        );
    }
    // Source truncates the name but hashes the entire passport: distinct IDs must conflict.
    let options = LabIdentityOptions {
        profile: IdentityProfile {
            architecture: FrameworkArchitecture::X86,
            culture: AsciiCulture::Invariant,
        },
        ngm_disable: true,
        zh_cn: false,
        max_passport_bytes: 128,
    };
    let prefix = "a".repeat(24);
    let left = derive_lab_identity(Some(&format!("{prefix}_one")), options).unwrap();
    let right = derive_lab_identity(Some(&format!("{prefix}_two")), options).unwrap();
    assert_eq!(left.nexon_id, right.nexon_id);
    assert_ne!(left.nexon_sn, right.nexon_sn);
    provision_game_user(&pool, left.nexon_sn, &left.nexon_id, 42, 0)
        .await
        .unwrap();
    assert!(matches!(
        provision_game_user(&pool, right.nexon_sn, &right.nexon_id, 42, 0).await,
        Err(Error::GameUserNameConflict)
    ));
    let x86 = derive_lab_identity(Some("ab"), options).unwrap();
    let x64 = derive_lab_identity(
        Some("ab"),
        LabIdentityOptions {
            profile: IdentityProfile {
                architecture: FrameworkArchitecture::X64,
                ..options.profile
            },
            ..options
        },
    )
    .unwrap();
    assert_ne!(x86.nexon_sn, x64.nexon_sn);
    provision_game_user(&pool, x86.nexon_sn, &x86.nexon_id, 42, 0)
        .await
        .unwrap();
    assert!(matches!(
        provision_game_user(&pool, x64.nexon_sn, &x64.nexon_id, 42, 0).await,
        Err(Error::GameUserNameConflict)
    ));
    for name in ["É", "a\n", "AAAAAAAAAAAAAAAAAAAAAAAAA"] {
        assert!(matches!(
            provision_game_user(&pool, 54321, name, 42, 0).await,
            Err(Error::GameUserName(_))
        ));
        // Direct SQL cannot bypass the name contract either.
        let error = sqlx::query(
            "INSERT INTO game_users(id,name,next_character_sn,service_id) VALUES(54321,$1,0,42)",
        )
        .bind(name)
        .execute(&pool)
        .await
        .unwrap_err();
        assert!(matches!(error, sqlx::Error::Database(ref db) if db.is_check_violation()));
    }
    for channel in [-1_i16, 256] {
        let error = sqlx::query(
            "INSERT INTO game_users(id,name,next_character_sn,channeling_code) VALUES(54321,'bounds',0,$1)"
        ).bind(channel).execute(&pool).await.unwrap_err();
        assert!(matches!(error, sqlx::Error::Database(ref db) if db.is_check_violation()));
    }
    let name_key: String = sqlx::query_scalar("SELECT name_key FROM game_users WHERE id=$1")
        .bind(i32::MIN)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(name_key, "mixed");
    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        accounts, 0,
        "game users are independent of foundation accounts"
    );
    postgres::check_ready(&pool).await.unwrap();
    sqlx::query("ALTER TABLE game_users RENAME COLUMN name_key TO broken_key")
        .execute(&pool)
        .await
        .unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn concurrent_game_user_provisioning_has_one_creator_and_preserves_committed_winner() {
    use vindictus_server::application::game_users::provision_game_user;
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..16 {
        let pool = pool.clone();
        tasks.spawn(async move {
            let name = if index % 2 == 0 { "RACE " } else { "race" };
            provision_game_user(&pool, 31415, name, 42, 6).await
        });
    }
    let mut creators = 0;
    while let Some(result) = tasks.join_next().await {
        let result = result.unwrap().unwrap();
        creators += usize::from(result.created);
        assert_eq!(result.user.id, 31415);
        assert_eq!(result.user.service_id, 42);
        assert_eq!(result.user.channeling_code, Some(6));
    }
    assert_eq!(creators, 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_users")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);

    // Force the second INSERT to take its snapshot before an uncommitted winner.
    // Observe its transactionid wait before committing/rolling back; no timing-only sleep.
    for commit_winner in [true, false] {
        let id = if commit_winner { 27182 } else { 16180 };
        let name = if commit_winner {
            "uncommitted"
        } else {
            "rolled_back"
        };
        let mut winner = pool.begin().await.unwrap();
        sqlx::query(
            "INSERT INTO game_users(id,name,next_character_sn,service_id,channeling_code) VALUES($1,$2,9,17,255)"
        ).bind(id).bind(name).execute(&mut *winner).await.unwrap();
        let marker = format!("game_wait_{}", Uuid::new_v4().simple());
        let waiting_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&std::env::var("TEST_DATABASE_URL").unwrap())
            .await
            .unwrap();
        let mut connection = waiting_pool.acquire().await.unwrap();
        sqlx::query(&format!("SET search_path TO {schema}"))
            .execute(&mut *connection)
            .await
            .unwrap();
        sqlx::query("SELECT set_config('application_name',$1,false)")
            .bind(&marker)
            .execute(&mut *connection)
            .await
            .unwrap();
        drop(connection);
        let task_pool = waiting_pool.clone();
        let waiting =
            tokio::spawn(async move { provision_game_user(&task_pool, id, name, 42, 0).await });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let blocked: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity
                     WHERE application_name=$1 AND wait_event='transactionid')",
                )
                .bind(&marker)
                .fetch_one(&admin)
                .await
                .unwrap();
                if blocked {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("losing insert must wait on the winner");
        if commit_winner {
            winner.commit().await.unwrap();
        } else {
            winner.rollback().await.unwrap();
        }
        let observed = waiting.await.unwrap().unwrap();
        assert_eq!(observed.created, !commit_winner);
        assert_eq!(
            observed.user.next_character_sn,
            if commit_winner { 9 } else { 0 }
        );
        assert_eq!(
            observed.user.service_id,
            if commit_winner { 17 } else { 42 }
        );
        assert_eq!(
            observed.user.channeling_code,
            if commit_winner { Some(255) } else { None }
        );
        waiting_pool.close().await;
    }
    cleanup_game_pool(admin, pool, schema).await;
}

#[derive(serde::Deserialize)]
struct UserPersistenceFixture {
    cases: Vec<UserPersistenceCase>,
}
#[derive(serde::Deserialize)]
struct UserPersistenceCase {
    id: i32,
    name: String,
    service_id: i32,
    channel_code: u8,
    next_character_sn: i32,
    channeling_code: Option<u8>,
}
#[derive(serde::Deserialize)]
struct SqlCollationFixture {
    cases: Vec<SqlCollationCase>,
}
#[derive(serde::Deserialize)]
struct SqlCollationCase {
    left_value: String,
    right_value: String,
    equal: bool,
    printable_ascii_contract: bool,
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn persisted_game_users_match_recovered_clr_defaults_and_sql_name_equality_oracles() {
    use vindictus_server::application::game_users::provision_game_user;
    let (admin, pool, schema) = isolated_game_pool().await;
    let users: UserPersistenceFixture =
        serde_json::from_str(include_str!("fixtures/framework-user.json")).unwrap();
    assert_eq!(users.cases.len(), 20);
    for case in users.cases {
        // Only this test's UUID schema is affected; source IDs intentionally repeat.
        sqlx::query("DELETE FROM game_users")
            .execute(&pool)
            .await
            .unwrap();
        let saved = provision_game_user(
            &pool,
            case.id,
            &case.name,
            case.service_id,
            case.channel_code,
        )
        .await
        .unwrap();
        assert!(saved.created);
        assert_eq!(saved.user.id, case.id);
        assert_eq!(saved.user.name, case.name);
        assert_eq!(saved.user.next_character_sn, case.next_character_sn);
        assert_eq!(saved.user.service_id, case.service_id);
        assert_eq!(
            saved.user.channeling_code,
            case.channeling_code.map(i16::from)
        );
    }
    let names: SqlCollationFixture = serde_json::from_str(include_str!(
        "../docs/evidence/database/collation-oracle.json"
    ))
    .unwrap();
    let mut checked = 0;
    for case in names
        .cases
        .into_iter()
        .filter(|case| case.printable_ascii_contract)
    {
        sqlx::query("DELETE FROM game_users")
            .execute(&pool)
            .await
            .unwrap();
        let saved = provision_game_user(&pool, 777, &case.left_value, 42, 0)
            .await
            .unwrap();
        assert!(saved.created);
        let other = provision_game_user(&pool, 777, &case.right_value, 42, 0).await;
        if case.equal {
            let other = other.unwrap();
            assert!(!other.created);
            assert_eq!(other.user.name, case.left_value);
        } else {
            assert!(matches!(other, Err(Error::GameUserIdConflict)));
        }
        checked += 1;
    }
    assert_eq!(checked, 11);
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn full_user_snapshots_preserve_all_source_fields_nulls_and_detect_conflicts() {
    use vindictus_server::{
        application::game_users::provision_game_user,
        domain::{game_user::GameUser, legacy_user_data::*},
        infrastructure::legacy_users::{insert_snapshot, read_snapshot},
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut snapshot = LegacyUserSnapshot {
        user: GameUser {
            id: -77,
            name: "Full_User ".into(),
            next_character_sn: i32::MAX,
            service_id: -1,
            channeling_code: Some(0),
        },
        details: LegacyUserDetails {
            last_login_ip: Some(LegacyBytes::from_bytes(vec![0, 216, 0, 0]).unwrap()),
            ip_lock_time: Some(SqlDateTime2::from_ticks(1).unwrap()),
            facebook_token: Some(LegacyBytes::from_bytes(vec![0, 128, 255]).unwrap()),
            is_dormant_user_event: Some(SqlDateTime::from_units300(-1).unwrap()),
            second_password: Some(LegacyBytes::from_bytes([0, 216].repeat(256)).unwrap()),
            second_password_fail_count: Some(i32::MIN),
            second_password_last_fail: Some(SqlDateTime2::from_ticks(SqlDateTime2::MAX).unwrap()),
            last_update: Some(SourceRowVersion::from_bytes(&[255; 8]).unwrap()),
            is_dormant_user: Some(SqlDateTime::from_units300(SqlDateTime::MIN).unwrap()),
            create_time: Some(SqlDateTime::from_units300(SqlDateTime::MAX).unwrap()),
        },
    };
    assert!(insert_snapshot(&pool, &snapshot).await.unwrap());
    assert!(!insert_snapshot(&pool, &snapshot).await.unwrap());
    assert!(read_snapshot(&pool, -77).await.unwrap().unwrap() == snapshot);
    // Provisioning does not overwrite imported counters, channel zero or details.
    let summary = provision_game_user(&pool, -77, "full_user", 99, 6)
        .await
        .unwrap();
    assert!(!summary.created);
    assert_eq!(summary.user.next_character_sn, i32::MAX);
    assert_eq!(summary.user.channeling_code, Some(0));
    assert!(read_snapshot(&pool, -77).await.unwrap().unwrap() == snapshot);
    snapshot.details.ip_lock_time = Some(SqlDateTime2::from_ticks(2).unwrap());
    assert!(matches!(
        insert_snapshot(&pool, &snapshot).await,
        Err(Error::LegacyUserSnapshotConflict)
    ));
    assert_eq!(
        read_snapshot(&pool, -77)
            .await
            .unwrap()
            .unwrap()
            .details
            .ip_lock_time
            .unwrap()
            .ticks(),
        1
    );
    snapshot.user.id = -78;
    assert!(matches!(
        insert_snapshot(&pool, &snapshot).await,
        Err(Error::LegacyUserSnapshotConflict)
    ));
    let nulls = LegacyUserSnapshot {
        user: GameUser {
            id: 0,
            name: "Nulls".into(),
            next_character_sn: 0,
            service_id: 42,
            channeling_code: None,
        },
        details: LegacyUserDetails::default(),
    };
    assert!(insert_snapshot(&pool, &nulls).await.unwrap());
    assert!(read_snapshot(&pool, 0).await.unwrap().unwrap() == nulls);
    let empty = LegacyUserSnapshot {
        user: GameUser {
            id: 1,
            name: "Empty".into(),
            next_character_sn: 0,
            service_id: 42,
            channeling_code: None,
        },
        details: LegacyUserDetails {
            second_password: Some(LegacyBytes::from_bytes(vec![]).unwrap()),
            facebook_token: Some(LegacyBytes::from_bytes(vec![]).unwrap()),
            ..Default::default()
        },
    };
    assert!(insert_snapshot(&pool, &empty).await.unwrap());
    assert!(read_snapshot(&pool, 1).await.unwrap().unwrap() == empty);
    let provisioned = provision_game_user(&pool, 2, "DefaultTime", 42, 0)
        .await
        .unwrap();
    assert!(provisioned.created);
    let default = read_snapshot(&pool, 2).await.unwrap().unwrap();
    assert!(default.details.create_time.is_some());
    assert!(
        default.details.last_update.is_none(),
        "do not fabricate a SQL Server counter"
    );
    assert!(read_snapshot(&pool, 123456).await.unwrap().is_none());
    for (column, value) in [
        ("ip_lock_time_ticks", "-1"),
        ("second_password_last_fail_ticks", "3155378976000000000"),
        ("create_time_units300", "76683386880000"),
        ("is_dormant_user_units300", "-1391644800001"),
        ("last_login_ip_utf16", "decode('00','hex')"),
        ("second_password_utf16", "decode(repeat('00',514),'hex')"),
        ("facebook_token_bytes", "decode(repeat('00',256),'hex')"),
        ("legacy_last_update", "decode('00','hex')"),
    ] {
        let error = sqlx::query(&format!(
            "UPDATE game_users SET {column}={value} WHERE id=2"
        ))
        .execute(&pool)
        .await
        .unwrap_err();
        assert!(matches!(error,sqlx::Error::Database(ref db) if db.is_check_violation()));
    }
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn migration_keeps_preexisting_create_time_unknown_and_sets_default_for_new_rows() {
    use sqlx::migrate::Migrator;
    let url = std::env::var("TEST_DATABASE_URL").unwrap();
    let admin = PgPoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    let schema = format!("test_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE SCHEMA {schema}"))
        .execute(&admin)
        .await
        .unwrap();
    let path = format!("SET search_path TO {schema}");
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .after_connect(move |connection, _| {
            let statement = path.clone();
            Box::pin(async move {
                sqlx::query(&statement).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    let previous = Migrator {
        migrations: std::borrow::Cow::Owned(
            postgres::MIGRATOR
                .iter()
                .filter(|m| m.version <= 2)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO game_users(id,name,next_character_sn,service_id) VALUES(10,'Before',0,42)",
    )
    .execute(&pool)
    .await
    .unwrap();
    postgres::migrate(&pool).await.unwrap();
    let unknown: Option<i64> =
        sqlx::query_scalar("SELECT create_time_units300 FROM game_users WHERE id=10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(unknown.is_none());
    sqlx::query(
        "INSERT INTO game_users(id,name,next_character_sn,service_id) VALUES(11,'After',0,42)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let (created,now):(i64,i64)=sqlx::query_as(
        "SELECT create_time_units300,floor((extract(epoch FROM clock_timestamp())+2208988800)*300+0.5)::bigint FROM game_users WHERE id=11"
    ).fetch_one(&pool).await.unwrap();
    assert!((0..3000).contains(&(now - created)));
    postgres::check_ready(&pool).await.unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn user_batch_is_atomic_on_late_conflict_and_verify_only_never_inserts() {
    use vindictus_server::{
        application::user_import::{ImportMode, import_users},
        infrastructure::{
            legacy_users::{insert_snapshot, read_snapshot},
            user_export::read_export,
        },
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let original = read_export(include_bytes!("fixtures/user-export.json").as_slice()).unwrap();
    assert!(matches!(
        import_users(&pool, &original, ImportMode::VerifyOnly).await,
        Err(Error::LegacyUserSnapshotConflict)
    ));
    assert!(read_snapshot(&pool, -200).await.unwrap().is_none());
    let mut conflict = read_export(include_bytes!("fixtures/user-export.json").as_slice()).unwrap();
    conflict[1].user.next_character_sn = 12;
    insert_snapshot(&pool, &conflict[1]).await.unwrap();
    // Sorted IDs mean the first row was inserted before the final row conflicts.
    assert!(matches!(
        import_users(&pool, &original, ImportMode::Apply).await,
        Err(Error::LegacyUserSnapshotConflict)
    ));
    assert!(read_snapshot(&pool, -200).await.unwrap().is_none());
    assert!(read_snapshot(&pool, -100).await.unwrap().unwrap() == conflict[1]);
    conflict[1].user.id = -300;
    conflict[1].user.name = "IMPORT_FULL".into();
    assert!(matches!(
        import_users(&pool, &conflict, ImportMode::Apply).await,
        Err(Error::UserExport)
    ));
    assert!(read_snapshot(&pool, -300).await.unwrap().is_none());
    conflict[1].user.name = "Another".into();
    conflict[1].user.id = -200;
    assert!(matches!(
        import_users(&pool, &conflict, ImportMode::Apply).await,
        Err(Error::UserExport)
    ));
    assert!(read_snapshot(&pool, -200).await.unwrap().is_none());
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn concurrent_user_batches_insert_once_and_preserve_all_fifteen_fields() {
    use vindictus_server::{
        application::user_import::{ImportMode, import_users},
        infrastructure::{legacy_users::read_snapshot, user_export::read_export},
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut tasks = tokio::task::JoinSet::new();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    for _ in 0..8 {
        let task_pool = pool.clone();
        let task_barrier = barrier.clone();
        tasks.spawn(async move {
            let batch =
                read_export(include_bytes!("fixtures/user-export.json").as_slice()).unwrap();
            task_barrier.wait().await;
            import_users(&task_pool, &batch, ImportMode::Apply)
                .await
                .unwrap()
        });
    }
    let mut inserted = 0;
    let mut operations = 0;
    while let Some(result) = tasks.join_next().await {
        let summary = result.unwrap();
        inserted += summary.inserted;
        assert_eq!(summary.verified, 2);
        assert_eq!(summary.inserted + summary.existing, 2);
        operations += 1;
    }
    assert_eq!(operations, 8);
    assert_eq!(inserted, 2);
    let batch = read_export(include_bytes!("fixtures/user-export.json").as_slice()).unwrap();
    let verified = import_users(&pool, &batch, ImportMode::VerifyOnly)
        .await
        .unwrap();
    assert_eq!(verified.inserted, 0);
    assert_eq!(verified.verified, 2);
    let replay = import_users(&pool, &batch, ImportMode::Apply)
        .await
        .unwrap();
    assert_eq!(replay.inserted, 0);
    for row in &batch {
        assert!(read_snapshot(&pool, row.user.id).await.unwrap().unwrap() == *row);
    }
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn user_ban_snapshots_and_uid_lookup_match_sql_predicate_and_preserve_source_fields() {
    use vindictus_server::{
        application::game_users::provision_game_user,
        domain::{legacy_user_data::*, login_policy::*},
        infrastructure::login_policy::{get_user_ban_info, insert_ban_snapshot},
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    provision_game_user(&pool, 42, "BanFixture", 42, 0)
        .await
        .unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/ban-policy-oracle.json"
    ))
    .unwrap();
    let now =
        SqlDateTime::from_units300(fixture["now_units300"].as_str().unwrap().parse().unwrap())
            .unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        sqlx::query("DELETE FROM game_user_bans")
            .execute(&pool)
            .await
            .unwrap();
        let ban = UserBan {
            name: case["name"].as_str().unwrap().into(),
            status: case["status"].as_i64().unwrap() as i32,
            expire_time: case["expiry_units300"]
                .as_str()
                .map(|s| SqlDateTime::from_units300(s.parse().unwrap()).unwrap()),
            reason: Some(LegacyBytes::from_bytes(vec![0, 216, 0, 0]).unwrap()),
            source_rowversion: Some(SourceRowVersion::from_bytes(&[255; 8]).unwrap()),
        };
        assert!(insert_ban_snapshot(&pool, &ban).await.unwrap());
        assert!(!insert_ban_snapshot(&pool, &ban).await.unwrap());
        let actual = get_user_ban_info(&pool, 42, now).await.unwrap();
        assert_eq!(actual.is_some(), case["selected"].as_bool().unwrap());
        if let Some(actual) = actual {
            assert!(actual == ban);
        }
    }
    assert!(
        get_user_ban_info(&pool, 12345, now)
            .await
            .unwrap()
            .is_none()
    );
    sqlx::query("DELETE FROM game_user_bans")
        .execute(&pool)
        .await
        .unwrap();
    let mut ban = UserBan {
        name: "BANFIXTURE ".into(),
        status: 2,
        expire_time: None,
        reason: Some(LegacyBytes::from_bytes([0, 216].repeat(256)).unwrap()),
        source_rowversion: Some(SourceRowVersion::from_bytes(&[254; 8]).unwrap()),
    };
    insert_ban_snapshot(&pool, &ban).await.unwrap();
    ban.status = 1;
    assert!(matches!(
        insert_ban_snapshot(&pool, &ban).await,
        Err(Error::LegacyBanConflict)
    ));
    let unchanged = get_user_ban_info(&pool, 42, now).await.unwrap().unwrap();
    assert_eq!(unchanged.status, 2);
    assert_eq!(unchanged.reason.unwrap().as_bytes().len(), 512);
    sqlx::query("ALTER TABLE game_user_bans RENAME TO unavailable_bans")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        get_user_ban_info(&pool, 42, now).await,
        Err(Error::Database(_))
    ));
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn ip_policy_compare_and_swap_has_one_winner_and_does_not_overwrite_source_rowversion() {
    use vindictus_server::{
        application::login_policy::{check_login_ip, update_expired_lock},
        domain::{legacy_user_data::*, login_policy::*},
        infrastructure::{
            legacy_users::{insert_snapshot, read_snapshot},
            login_policy::read_ip_state,
            user_export::read_export,
        },
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut batch = read_export(include_bytes!("fixtures/user-export.json").as_slice()).unwrap();
    batch[0].details.last_login_ip = None;
    batch[0].details.ip_lock_time = None;
    insert_snapshot(&pool, &batch[0]).await.unwrap();
    let id = batch[0].user.id;
    let now = SqlDateTime2::from_ticks(638_448_048_000_000_000).unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(8));
    for _ in 0..8 {
        let task_pool = pool.clone();
        let task_barrier = barrier.clone();
        tasks.spawn(async move {
            let request = IpRequest {
                connection_ip: "192.0.2.1".parse().unwrap(),
                secured: false,
                has_cheat_permission: false,
                lock_minutes_feature: 0,
                clock: ApplicationClockSamples {
                    set_lock_now: now,
                    check_lock_now: now,
                },
            };
            task_barrier.wait().await;
            check_login_ip(&task_pool, id, 0, &request).await
        });
    }
    let mut won = 0;
    let mut refused = 0;
    while let Some(result) = tasks.join_next().await {
        match result.unwrap() {
            Ok(value) => {
                assert_eq!(value.revision, 1);
                won += 1;
            }
            Err(Error::GameUserRevisionConflict) => refused += 1,
            Err(error) => panic!("unexpected policy error: {error}"),
        }
    }
    assert_eq!((won, refused), (1, 7));
    let mut request = IpRequest {
        connection_ip: "192.0.2.1".parse().unwrap(),
        secured: false,
        has_cheat_permission: false,
        lock_minutes_feature: -1,
        clock: ApplicationClockSamples {
            set_lock_now: now,
            check_lock_now: now,
        },
    };
    assert_eq!(
        check_login_ip(&pool, id, 1, &request)
            .await
            .unwrap()
            .revision,
        1,
        "no-op does not issue UPDATE"
    );
    request.connection_ip = "192.0.2.2".parse().unwrap();
    let locked = check_login_ip(&pool, id, 1, &request).await.unwrap();
    assert_eq!(locked.revision, 2);
    assert_eq!(locked.evaluation.lock_seconds, 600);
    request.secured = true;
    let secured = check_login_ip(&pool, id, 2, &request).await.unwrap();
    assert_eq!(secured.revision, 3);
    assert!(secured.evaluation.state.lock_until.is_none());
    sqlx::query("UPDATE game_users SET service_id=99 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(read_ip_state(&pool, id).await.unwrap().revision, 4);
    assert!(matches!(
        check_login_ip(&pool, id, 3, &request).await,
        Err(Error::GameUserRevisionConflict)
    ));
    assert!(
        read_snapshot(&pool, id)
            .await
            .unwrap()
            .unwrap()
            .details
            .last_update
            == batch[0].details.last_update
    );
    let reset = sqlx::query("UPDATE game_users SET local_revision=0 WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap_err();
    assert!(matches!(reset,sqlx::Error::Database(ref db) if db.is_check_violation()));
    sqlx::query("ALTER TABLE game_users DISABLE TRIGGER game_user_revision")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        postgres::check_ready(&pool).await,
        Err(Error::SchemaNotReady)
    ));
    sqlx::query("ALTER TABLE game_users ENABLE TRIGGER game_user_revision")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        check_login_ip(&pool, 99999, 0, &request).await,
        Err(Error::GameUserMissing)
    ));
    let expiry = now.ticks() + 100;
    batch[1].details.ip_lock_time = Some(SqlDateTime2::from_ticks(expiry).unwrap());
    insert_snapshot(&pool, &batch[1]).await.unwrap();
    assert_eq!(
        update_expired_lock(&pool, batch[1].user.id, 0, request.connection_ip, now)
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        update_expired_lock(
            &pool,
            batch[1].user.id,
            0,
            request.connection_ip,
            SqlDateTime2::from_ticks(expiry).unwrap()
        )
        .await
        .unwrap(),
        1
    );
    assert_eq!(
        update_expired_lock(&pool, batch[1].user.id, 1, request.connection_ip, now)
            .await
            .unwrap(),
        1
    );
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn login_policy_migration_preserves_existing_source_values_and_revision_across_replay() {
    use sqlx::migrate::Migrator;
    let (admin, pool, schema) = isolated_game_pool().await;
    sqlx::raw_sql("DROP TABLE game_login_account_effect_runs,game_friend_loyalty,game_user_achievements;DELETE FROM _sqlx_migrations WHERE version=6;DROP TABLE game_limited_accounts; DROP FUNCTION limited_ascii_key(bytea); DELETE FROM _sqlx_migrations WHERE version=5")
        .execute(&pool).await.unwrap();
    // Recreate only our disposable isolated schema to exercise the previous version.
    sqlx::query("DROP TABLE game_user_bans")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP TRIGGER game_user_revision ON game_users")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DROP FUNCTION bump_game_user_revision()")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("ALTER TABLE game_users DROP COLUMN local_revision")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version=4")
        .execute(&pool)
        .await
        .unwrap();
    let previous = Migrator {
        migrations: std::borrow::Cow::Owned(
            postgres::MIGRATOR
                .iter()
                .filter(|m| m.version <= 3)
                .cloned()
                .collect(),
        ),
        ..Migrator::DEFAULT
    };
    previous.run(&pool).await.unwrap();
    sqlx::query("INSERT INTO game_users(id,name,next_character_sn,service_id,legacy_last_update) VALUES(10,'BeforePolicy',0,42,decode('0102030405060708','hex'))").execute(&pool).await.unwrap();
    postgres::migrate(&pool).await.unwrap();
    let before: i64 = sqlx::query_scalar("SELECT local_revision FROM game_users WHERE id=10")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, 0);
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=10")
        .execute(&pool)
        .await
        .unwrap();
    postgres::migrate(&pool).await.unwrap();
    let (revision, token): (i64, Vec<u8>) =
        sqlx::query_as("SELECT local_revision,legacy_last_update FROM game_users WHERE id=10")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revision, 1);
    assert_eq!(token, [1, 2, 3, 4, 5, 6, 7, 8]);
    postgres::check_ready(&pool).await.unwrap();
    cleanup_game_pool(admin, pool, schema).await;
}

fn lab_admission() -> vindictus_server::application::frontend_session::LabAdmission {
    use vindictus_server::{application::frontend_session::LabAdmission, protocol::identity::*};
    LabAdmission {
        identity_options: LabIdentityOptions {
            profile: IdentityProfile {
                architecture: FrameworkArchitecture::X86,
                culture: AsciiCulture::Invariant,
            },
            ngm_disable: true,
            zh_cn: false,
            max_passport_bytes: 128,
        },
        allow_cheat: false,
        allow_during_maintenance: false,
        is_online: true,
    }
}
fn begin_lab(
    registry: &vindictus_server::application::session_registry::SessionRegistry,
    passport: &str,
    config: &vindictus_server::application::frontend_session::LabAdmission,
) -> (
    vindictus_server::application::frontend_session::FrontendSession,
    vindictus_server::application::frontend_session::AttemptToken,
) {
    use std::time::{Duration, Instant};
    let mut session =
        vindictus_server::application::frontend_session::FrontendSession::new(registry);
    let now = Instant::now();
    let token = session.begin(now, Duration::from_secs(10)).unwrap();
    session
        .admit_lab(
            token,
            now,
            Some(passport),
            "127.0.0.1".parse().unwrap(),
            config,
        )
        .unwrap();
    (session, token)
}
fn lab_clocks() -> vindictus_server::application::lab_login::PolicyClock {
    use vindictus_server::{
        application::lab_login::PolicyClock,
        domain::{legacy_user_data::*, login_policy::*},
    };
    PolicyClock {
        application: ApplicationClockSamples {
            set_lock_now: SqlDateTime2::from_ticks(638_500_000_000_000_000).unwrap(),
            check_lock_now: SqlDateTime2::from_ticks(638_500_000_000_000_000).unwrap(),
        },
        sql_local_now: SqlDateTime::from_units300(1_170_000_000_000).unwrap(),
    }
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn existing_lab_session_checks_identity_ban_then_ip_and_rejects_stale_revision() {
    use vindictus_server::{
        application::{
            frontend_session::{SessionError, SessionPhase},
            game_users::provision_game_user,
            lab_login::{LabPolicyError, evaluate_existing_lab_user},
        },
        domain::{
            legacy_user_data::LegacyBytes,
            login_policy::{IpState, UserBan},
            passport::LoginRejection,
        },
        infrastructure::login_policy::{insert_ban_snapshot, read_ip_state},
        protocol::identity::derive_lab_identity,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry =
        vindictus_server::application::session_registry::SessionRegistry::with_capacity(32)
            .unwrap();
    let cfg = lab_admission();
    let identity = derive_lab_identity(Some("session_policy"), cfg.identity_options).unwrap();
    provision_game_user(&pool, identity.nexon_sn, &identity.nexon_id, 42, 0)
        .await
        .unwrap();
    let (mut session, token) = begin_lab(&registry, "session_policy", &cfg);
    let result = evaluate_existing_lab_user(&mut session, token, &pool, 0, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(result.revision, 1);
    assert_eq!(result.client_lock_seconds, 0);
    assert_eq!(session.phase(), SessionPhase::PoliciesAccepted);
    session.close();
    let (mut stale, token) = begin_lab(&registry, "session_policy", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut stale, token, &pool, 0, lab_clocks(), 10).await,
        Err(LabPolicyError::Storage(Error::GameUserRevisionConflict))
    ));
    assert_eq!(stale.phase(), SessionPhase::Initial);
    let changed = IpState {
        last_login_ip: Some(
            LegacyBytes::from_bytes(
                "10.0.0.8"
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        ),
        lock_until: None,
    };
    sqlx::query("UPDATE game_users SET last_login_ip_utf16=$2,ip_lock_time_ticks=NULL WHERE id=$1")
        .bind(identity.nexon_sn)
        .bind(changed.last_login_ip.as_ref().unwrap().as_bytes())
        .execute(&pool)
        .await
        .unwrap();
    let (mut locked, token) = begin_lab(&registry, "session_policy", &cfg);
    let result = evaluate_existing_lab_user(&mut locked, token, &pool, 2, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(result.client_lock_seconds, 600); // loopback alone does not get legacy Cheat's 60 seconds.
    locked.close();
    let mut cheat_cfg = lab_admission();
    cheat_cfg.allow_cheat = true;
    let (mut cheat, token) = begin_lab(&registry, "session_policy", &cheat_cfg);
    let result = evaluate_existing_lab_user(&mut cheat, token, &pool, 3, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(result.client_lock_seconds, 60);
    insert_ban_snapshot(
        &pool,
        &UserBan {
            name: identity.nexon_id.clone(),
            status: 1,
            expire_time: None,
            reason: None,
            source_rowversion: None,
        },
    )
    .await
    .unwrap();
    let before = read_ip_state(&pool, identity.nexon_sn).await.unwrap();
    cheat.close();
    let (mut banned, token) = begin_lab(&registry, "session_policy", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut banned, token, &pool, before.revision, lab_clocks(), 10)
            .await,
        Err(LabPolicyError::Session(SessionError::Rejected(
            LoginRejection::Banned
        )))
    ));
    assert_eq!(banned.phase(), SessionPhase::Initial);
    let after = read_ip_state(&pool, identity.nexon_sn).await.unwrap();
    assert_eq!(before.revision, after.revision);
    assert!(before.state == after.state);
    let (mut unknown, token) = begin_lab(&registry, "not_provisioned", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut unknown, token, &pool, 0, lab_clocks(), 10).await,
        Err(LabPolicyError::Session(SessionError::Rejected(
            LoginRejection::NoUserInfo
        )))
    ));
    assert_eq!(unknown.phase(), SessionPhase::Initial);
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn lab_session_refuses_identity_collision_database_failure_and_blocked_deadline() {
    use std::time::{Duration, Instant};
    use vindictus_server::{
        application::{
            frontend_session::{FrontendSession, SessionError, SessionPhase},
            game_users::provision_game_user,
            lab_login::{LabPolicyError, evaluate_existing_lab_user},
        },
        domain::passport::LoginRejection,
        protocol::identity::derive_lab_identity,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry =
        vindictus_server::application::session_registry::SessionRegistry::with_capacity(32)
            .unwrap();
    let cfg = lab_admission();
    let identity = derive_lab_identity(Some("collision_session"), cfg.identity_options).unwrap();
    provision_game_user(&pool, identity.nexon_sn, "different_name", 42, 0)
        .await
        .unwrap();
    let (mut collision, token) = begin_lab(&registry, "collision_session", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut collision, token, &pool, 0, lab_clocks(), 10).await,
        Err(LabPolicyError::Session(SessionError::Rejected(
            LoginRejection::NoUserInfo
        )))
    ));
    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_users IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let now = Instant::now();
    let mut timed = FrontendSession::new(&registry);
    let token = timed.begin(now, Duration::from_millis(100)).unwrap();
    timed
        .admit_lab(
            token,
            now,
            Some("collision_session"),
            "127.0.0.1".parse().unwrap(),
            &cfg,
        )
        .unwrap();
    assert!(matches!(
        evaluate_existing_lab_user(&mut timed, token, &pool, 0, lab_clocks(), 10).await,
        Err(LabPolicyError::Timeout)
    ));
    assert_eq!(timed.phase(), SessionPhase::Closed);
    blocker.rollback().await.unwrap();
    pool.close().await;
    let (mut unavailable, token) = begin_lab(&registry, "collision_session", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut unavailable, token, &pool, 0, lab_clocks(), 10).await,
        Err(LabPolicyError::Storage(_))
    ));
    assert_eq!(unavailable.phase(), SessionPhase::Initial);
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn exclusive_lab_session_survives_duplicate_reconnect_and_cancellation_without_mutation() {
    use vindictus_server::{
        application::{
            frontend_session::{FrontendSession, SessionError, SessionPhase},
            game_users::provision_game_user,
            lab_login::evaluate_existing_lab_user,
            session_registry::SessionRegistry,
        },
        domain::passport::LoginRejection,
        infrastructure::login_policy::read_ip_state,
        protocol::identity::derive_lab_identity,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(4).unwrap();
    let cfg = lab_admission();
    let identity = derive_lab_identity(Some("exclusive_session"), cfg.identity_options).unwrap();
    provision_game_user(&pool, identity.nexon_sn, &identity.nexon_id, 42, 0)
        .await
        .unwrap();
    let (mut first, token) = begin_lab(&registry, "exclusive_session", &cfg);
    let mut contender = FrontendSession::new(&registry);
    let now = std::time::Instant::now();
    let loser = contender
        .begin(now, std::time::Duration::from_secs(10))
        .unwrap();
    assert_eq!(
        contender.admit_lab(
            loser,
            now,
            Some("exclusive_session"),
            "127.0.0.1".parse().unwrap(),
            &cfg
        ),
        Err(SessionError::Rejected(LoginRejection::DuplicateLogin))
    );
    assert_eq!(
        read_ip_state(&pool, identity.nexon_sn)
            .await
            .unwrap()
            .revision,
        0
    );
    let accepted = evaluate_existing_lab_user(&mut first, token, &pool, 0, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(accepted.revision, 1);
    assert_eq!(first.phase(), SessionPhase::PoliciesAccepted);
    assert_eq!(registry.active_count().unwrap(), 1);
    first.close();
    let (mut reconnect, token) = begin_lab(&registry, "exclusive_session", &cfg);
    first.close();
    drop(first);
    drop(contender); // obsolete cleanup cannot remove reconnect's reservation.
    let accepted = evaluate_existing_lab_user(&mut reconnect, token, &pool, 1, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(accepted.revision, 1); // exact same IP makes this a persisted no-op.
    assert_eq!(registry.active_count().unwrap(), 1);
    drop(reconnect);
    assert_eq!(registry.active_count().unwrap(), 0);
    let before = read_ip_state(&pool, identity.nexon_sn).await.unwrap();
    // Exhaust this isolated pool so evaluation is polled and really suspended on acquisition.
    let mut held = Vec::new();
    for _ in 0..8 {
        held.push(pool.acquire().await.unwrap());
    }
    let (mut cancelled, token) = begin_lab(&registry, "exclusive_session", &cfg);
    let mut future = Box::pin(evaluate_existing_lab_user(
        &mut cancelled,
        token,
        &pool,
        1,
        lab_clocks(),
        10,
    ));
    tokio::select! {
        biased;
        _ = &mut future => panic!("blocked pool unexpectedly allowed evaluation"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {}
    }
    drop(future); // caller cancellation, session owner remains alive.
    assert_eq!(cancelled.phase(), SessionPhase::Closed);
    assert_eq!(registry.active_count().unwrap(), 0);
    drop(held);
    let after = read_ip_state(&pool, identity.nexon_sn).await.unwrap();
    assert_eq!(before.revision, after.revision);
    assert!(before.state == after.state);
    let (mut replacement, token) = begin_lab(&registry, "exclusive_session", &cfg);
    evaluate_existing_lab_user(&mut replacement, token, &pool, 1, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(replacement.phase(), SessionPhase::PoliciesAccepted);
    drop(replacement);
    assert_eq!(registry.active_count().unwrap(), 0);
    cleanup_game_pool(admin, pool, schema).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn limited_accounts_preserve_bytes_and_duplicates_and_match_sql_ascii_oracle() {
    use vindictus_server::{
        domain::legacy_user_data::LegacyBytes,
        infrastructure::limited_accounts::{insert_snapshot, is_limited},
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let oracle: serde_json::Value = serde_json::from_str(include_str!(
        "../docs/evidence/database/limited-account-oracle.json"
    ))
    .unwrap();
    assert_eq!(oracle["source_rows"], 0);
    assert!(!is_limited(&pool, "LimitedFixture").await.unwrap());
    let bytes = LegacyBytes::<24, true>::from_bytes(
        "LimitedFixture"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    insert_snapshot(&pool, &bytes).await.unwrap();
    for case in oracle["cases"].as_array().unwrap() {
        assert_eq!(
            is_limited(&pool, case["name"].as_str().unwrap())
                .await
                .unwrap(),
            case["matched"].as_bool().unwrap()
        );
    }
    let duplicate = LegacyBytes::<24, true>::from_bytes(
        "LIMITEDFIXTURE "
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    insert_snapshot(&pool, &duplicate).await.unwrap();
    assert!(matches!(
        is_limited(&pool, "LimitedFixture").await,
        Err(Error::LimitedAccountDuplicate)
    ));
    let raw = LegacyBytes::<24, true>::from_bytes(vec![0, 216, 0, 0]).unwrap();
    let id = insert_snapshot(&pool, &raw).await.unwrap();
    let stored: Vec<u8> =
        sqlx::query_scalar("SELECT nexon_id_utf16 FROM game_limited_accounts WHERE local_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored, raw.as_bytes());
    assert!(matches!(
        is_limited(&pool, "Other").await,
        Err(Error::LimitedAccountUnsupported)
    ));
    for bytes in [vec![0; 49], vec![0; 50]] {
        assert!(
            sqlx::query("INSERT INTO game_limited_accounts(nexon_id_utf16) VALUES($1)")
                .bind(bytes)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    assert!(
        sqlx::query("INSERT INTO game_limited_accounts(nexon_id_utf16) VALUES(NULL)")
            .execute(&pool)
            .await
            .is_err()
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn limited_accounts_upgrade_and_policy_flag_preserve_users_and_refuse_unknown_policy() {
    use vindictus_server::{
        application::{
            frontend_session::SessionPhase,
            game_users::provision_game_user,
            lab_login::{LabPolicyError, evaluate_existing_lab_user},
            session_registry::SessionRegistry,
        },
        domain::legacy_user_data::LegacyBytes,
        infrastructure::{limited_accounts::insert_snapshot, login_policy::read_ip_state},
        protocol::identity::derive_lab_identity,
    };
    let (admin, pool, schema) = isolated_game_pool().await;
    let registry = SessionRegistry::with_capacity(4).unwrap();
    let cfg = lab_admission();
    let identity = derive_lab_identity(Some("limited_policy"), cfg.identity_options).unwrap();
    provision_game_user(&pool, identity.nexon_sn, &identity.nexon_id, 42, 0)
        .await
        .unwrap();
    sqlx::raw_sql("DROP TABLE game_login_account_effect_runs,game_friend_loyalty,game_user_achievements;DELETE FROM _sqlx_migrations WHERE version=6;DROP TABLE game_limited_accounts;DROP FUNCTION limited_ascii_key(bytea);DELETE FROM _sqlx_migrations WHERE version=5")
        .execute(&pool).await.unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    postgres::migrate(&pool).await.unwrap();
    postgres::check_ready(&pool).await.unwrap();
    assert_eq!(
        read_ip_state(&pool, identity.nexon_sn)
            .await
            .unwrap()
            .revision,
        0
    );
    let row = LegacyBytes::<24, true>::from_bytes(
        identity
            .nexon_id
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    insert_snapshot(&pool, &row).await.unwrap();
    let (mut session, token) = begin_lab(&registry, "limited_policy", &cfg);
    let accepted = evaluate_existing_lab_user(&mut session, token, &pool, 0, lab_clocks(), 10)
        .await
        .unwrap();
    assert_eq!(accepted.limited, 1);
    assert_eq!(session.phase(), SessionPhase::PoliciesAccepted);
    session.close();
    insert_snapshot(&pool, &row).await.unwrap();
    let (mut duplicate, token) = begin_lab(&registry, "limited_policy", &cfg);
    assert!(matches!(
        evaluate_existing_lab_user(&mut duplicate, token, &pool, 1, lab_clocks(), 10).await,
        Err(LabPolicyError::Storage(Error::LimitedAccountDuplicate))
    ));
    assert_eq!(duplicate.phase(), SessionPhase::Initial);
    assert_eq!(registry.active_count().unwrap(), 0);
    postgres::migrate(&pool).await.unwrap();
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_limited_accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 2); // replay does not collapse source duplicates.
    cleanup_game_pool(admin, pool, schema).await;
}
#[path = "support/login_account_effects.rs"]
mod account_effects;
