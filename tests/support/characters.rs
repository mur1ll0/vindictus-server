use super::{cleanup_game_pool, isolated_game_pool};
use serde_json::Value;
use std::sync::Arc;
use vindictus_server::{
    error::Error,
    infrastructure::{character_export::read_export, characters::*, postgres},
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("../fixtures/character-bundle.json")).unwrap()
}
fn bundle(v: &Value) -> vindictus_server::domain::character_data::CharacterBundle {
    read_export(serde_json::to_vec(v).unwrap().as_slice()).unwrap()
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn character_bundle_preserves_all_64_columns_and_source_login_queries() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let data = bundle(&fixture());
    assert_eq!(
        import_bundle(&pool, &data, false)
            .await
            .unwrap()
            .inserted
            .total(),
        13
    );
    for r in &data.characters {
        assert!(read_characters(&pool, r.id).await.unwrap().as_ref() == Some(r));
    }
    for r in &data.character_info {
        assert!(read_character_info(&pool, r.id).await.unwrap().as_ref() == Some(r));
    }
    for r in &data.connections {
        assert!(read_connections(&pool, r.id).await.unwrap().as_ref() == Some(r));
    }
    for r in &data.queued_items {
        assert!(read_queued_items(&pool, r.row_id).await.unwrap().as_ref() == Some(r));
    }
    assert_eq!(
        import_bundle(&pool, &data, false)
            .await
            .unwrap()
            .inserted
            .total(),
        0
    );
    assert_eq!(
        import_bundle(&pool, &data, true)
            .await
            .unwrap()
            .verified
            .total(),
        13
    );
    assert_eq!(
        get_characters(&pool, 7)
            .await
            .unwrap()
            .iter()
            .map(|r| r.id)
            .collect::<Vec<_>>(),
        [3, 1, 2]
    );
    assert!(get_characters(&pool, -1).await.unwrap().is_empty());
    assert!(
        get_latest_connected_date(&pool, -1)
            .await
            .unwrap()
            .is_none()
    );
    let oracle: Value = serde_json::from_str(include_str!(
        "../../docs/evidence/database/character-login-oracle.json"
    ))
    .unwrap();
    assert_eq!(oracle["listed_ids"], serde_json::json!(["3", "1", "2"]));
    assert_eq!(
        get_latest_connected_date(&pool, 7)
            .await
            .unwrap()
            .unwrap()
            .units300(),
        oracle["bundle_latest"]
            .as_str()
            .unwrap()
            .parse::<i64>()
            .unwrap()
    );
    sqlx::query("UPDATE game_character_info SET uid=9 WHERE id=1")
        .execute(&pool)
        .await
        .unwrap();
    for case in oracle["latest_cases"].as_array().unwrap() {
        sqlx::query("UPDATE game_last_connect_info SET last_connect_date_units300=$1,last_cafe_logout_units300=$2 WHERE id=1")
 .bind(case["connect"].as_str().unwrap().parse::<i64>().unwrap()).bind(case["logout"].as_str().map(|v|v.parse::<i64>().unwrap())).execute(&pool).await.unwrap();
        assert_eq!(
            get_latest_connected_date(&pool, 9)
                .await
                .unwrap()
                .map(|d| d.units300()),
            case["result"].as_str().map(|v| v.parse::<i64>().unwrap())
        );
    }
    for q in [
        "UPDATE game_character_info SET class=256 WHERE id=1",
        "UPDATE game_character_info SET order_num=-1 WHERE id=1",
        "UPDATE game_character_info SET name_utf16=decode('00','hex') WHERE id=1",
        "UPDATE game_character_info SET legacy_last_update=decode('00','hex') WHERE id=1",
        "UPDATE game_queued_items SET mail_title_utf16=decode(repeat('00',514),'hex') WHERE row_id=1",
        "UPDATE game_last_connect_info SET last_connect_date_units300=76683386880000 WHERE id=1",
    ] {
        assert!(sqlx::query(q).execute(&pool).await.is_err());
    }
    // Source CASCADE/NO ACTION behavior, exercised only in this disposable schema.
    sqlx::query("UPDATE game_characters SET id=33 WHERE id=3")
        .execute(&pool)
        .await
        .unwrap();
    assert!(read_character_info(&pool, 33).await.unwrap().is_some());
    assert!(
        sqlx::query("UPDATE game_characters SET id=11 WHERE id=1")
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(read_characters(&pool, 1).await.unwrap().is_some());
    sqlx::query("DELETE FROM game_characters WHERE id=2")
        .execute(&pool)
        .await
        .unwrap();
    assert!(read_character_info(&pool, 2).await.unwrap().is_none());
    assert!(read_connections(&pool, 2).await.unwrap().is_none());
    assert!(read_queued_items(&pool, 2).await.unwrap().is_some());
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn character_bundle_conflicts_and_missing_fk_roll_back_the_entire_batch() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let mut orphan = fixture();
    orphan["connections"][0]["id"] = "999".into();
    assert!(import_bundle(&pool, &bundle(&orphan), false).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM game_characters")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    let data = bundle(&fixture());
    import_bundle(&pool, &data, false).await.unwrap();
    let mut changed = fixture();
    changed["characters"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"5"}));
    changed["source_counts"]["characters"] = 5.into();
    changed["queued_items"][1]["count"] = 1.into();
    assert!(matches!(
        import_bundle(&pool, &bundle(&changed), false).await,
        Err(Error::CharacterSnapshotConflict)
    ));
    assert!(read_characters(&pool, 5).await.unwrap().is_none());
    assert!(read_queued_items(&pool, 2).await.unwrap().as_ref() == Some(&data.queued_items[1]));
    for table in ["character_info", "connections", "queued_items"] {
        for (name, old) in fixture()[table][0].as_object().unwrap() {
            if name == "id" || name == "row_id" {
                continue;
            }
            let new = if name == "legacy_last_update" {
                Value::from("0807060504030201")
            } else if old.is_null() {
                if name.ends_with("_utf16") {
                    Value::from("")
                } else if ["order_num", "channel", "selected_title", "today_play_time"]
                    .contains(&name.as_str())
                {
                    Value::from(0)
                } else {
                    Value::from("0")
                }
            } else if old.is_boolean() {
                Value::from(!old.as_bool().unwrap())
            } else if let Some(n) = old.as_i64() {
                Value::from(if n == 0 { 1 } else { 0 })
            } else if name.ends_with("_utf16") {
                Value::from("4100")
            } else {
                Value::from("0")
            };
            let mut v = fixture();
            v[table][0][name] = new;
            assert!(
                matches!(
                    import_bundle(&pool, &bundle(&v), false).await,
                    Err(Error::CharacterSnapshotConflict)
                ),
                "{table}.{name}"
            );
        }
    }
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn character_bundle_concurrent_replays_preserve_one_atomic_copy() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let data = Arc::new(bundle(&fixture()));
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let data = data.clone();
        let pool = pool.clone();
        tasks.push(tokio::spawn(async move {
            import_bundle(&pool, &data, false)
                .await
                .unwrap()
                .inserted
                .total()
        }));
    }
    let mut totals = Vec::new();
    for t in tasks {
        totals.push(t.await.unwrap());
    }
    assert_eq!(totals.iter().filter(|&&v| v == 13).count(), 1);
    assert_eq!(totals.iter().filter(|&&v| v == 0).count(), 15);
    assert_eq!(
        import_bundle(&pool, &data, true)
            .await
            .unwrap()
            .verified
            .total(),
        13
    );
    cleanup_game_pool(admin, pool, schema).await;
}
#[tokio::test]
#[ignore = "requires isolated PostgreSQL"]
async fn character_schema_upgrade_preserves_user_and_requires_new_columns() {
    let (admin, pool, schema) = isolated_game_pool().await;
    let data = bundle(&fixture());
    import_bundle(&pool, &data, false).await.unwrap();
    sqlx::query(
        "INSERT INTO game_users(id,name,next_character_sn,service_id) VALUES(7,'Fixture',0,0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let before: String =
        sqlx::query_scalar("SELECT md5(row(u.*)::text) FROM game_users u WHERE id=7")
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::raw_sql("DROP TABLE game_titles,game_title_goal_progress,game_title_presets,game_micro_play_status,game_character_mission_info,game_mission_assignments,game_character_state_preservation;DELETE FROM _sqlx_migrations WHERE version=20;DROP TABLE game_skills,game_sp_skills,game_skill_identity;DELETE FROM _sqlx_migrations WHERE version=19;DROP TABLE game_items,game_item_attributes,game_equippables,game_item_storage_status,game_item_identity;DROP FUNCTION item_attribute_ascii_key(bytea);DELETE FROM _sqlx_migrations WHERE version=18;DROP TABLE game_costumes,game_costume_identity;DELETE FROM _sqlx_migrations WHERE version=17;DROP TABLE game_character_ledger01,game_character_ledger02;DELETE FROM _sqlx_migrations WHERE version=16;DROP TABLE game_item_collections,game_quest_collections,game_mailboxes,game_story_entries,game_my_settings;DELETE FROM _sqlx_migrations WHERE version=15;DROP TABLE game_reserved_names,game_reserved_titles,game_character_name_logs;DELETE FROM _sqlx_migrations WHERE version=14;DROP TABLE game_lab_account_origins;DELETE FROM _sqlx_migrations WHERE version=13;DROP TABLE game_home_cafe_bonus;DELETE FROM _sqlx_migrations WHERE version=12;DROP TABLE game_user_care_info,game_user_products,game_user_info;DELETE FROM _sqlx_migrations WHERE version=11;DROP TABLE game_login_reward_runs;DELETE FROM _sqlx_migrations WHERE version=10;DROP TABLE game_dormant_item_runs,game_queue_identity;DELETE FROM _sqlx_migrations WHERE version=9;DROP TABLE game_last_connect_info,game_character_info,game_characters,game_queued_items;DELETE FROM _sqlx_migrations WHERE version=8;DELETE FROM _sqlx_migrations WHERE version=7").execute(&pool).await.unwrap();
    assert!(postgres::check_ready(&pool).await.is_err());
    postgres::migrate(&pool).await.unwrap();
    postgres::check_ready(&pool).await.unwrap();
    let after: String =
        sqlx::query_scalar("SELECT md5(row(u.*)::text) FROM game_users u WHERE id=7")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    import_bundle(&pool, &data, false).await.unwrap();
    postgres::migrate(&pool).await.unwrap();
    assert_eq!(
        import_bundle(&pool, &data, true)
            .await
            .unwrap()
            .verified
            .total(),
        13
    );
    sqlx::query("ALTER TABLE game_character_info RENAME COLUMN quote_utf16 TO missing_quote")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        postgres::check_ready(&pool).await,
        Err(Error::SchemaNotReady)
    ));
    cleanup_game_pool(admin, pool, schema).await;
}
