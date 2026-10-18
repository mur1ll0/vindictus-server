use super::{cleanup_game_pool, isolated_game_pool, name_check_common as common};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use vindictus_server::{
    application::game_users::provision_game_user,
    domain::feature_policy::FeaturePolicy,
    infrastructure::{
        character_creation_core::{self, CreationCoreRequest},
        character_lobby::UserKey,
        character_skills, fiona_character_state as state,
        fiona_initializer_profile::Profile,
        fiona_managers::{self as managers, ManagerError},
        item_catalog::{CatalogManifest, Package},
    },
};
mod profile_fixture {
    include!("../fixtures/initializer_profile_support.rs");
}
fn owner() -> UserKey<'static> {
    UserKey {
        uid: 31,
        name: "Owner",
        revision: 0,
    }
}
fn state_export() -> Value {
    json!({"format_version":1,"source":"study_heroes.character-initialization","titles":[],"goals":[],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"micro":[],"missions":[],"assignments":[]})
}
fn profile(ratio: &str) -> Profile {
    let mut raw = profile_fixture::synthetic();
    raw["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
    ));
    raw["features"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|v| v["key"] == "AccountTitleStatRatio")
        .unwrap()["value"] = json!(ratio);
    profile_fixture::read(&raw).unwrap()
}
fn catalog() -> Package {
    Package {
        manifest: CatalogManifest {
            database_sha256: "a".repeat(64),
            config_sha256: "b".repeat(64),
            region: "synthetic".into(),
            override_region: None,
            equip_rows: 0,
            class_rows: 0,
            extra_rows: 0,
        },
        policy: FeaturePolicy::new([("CurrentVer".into(), Some("synthetic-current".into()))])
            .unwrap(),
        equips: vec![],
        classes: vec![],
        extras: vec![],
    }
}
async fn counts(p: &PgPool) -> (i64, i64, i64, i64, i64) {
    sqlx::query_as("SELECT (SELECT COUNT(*) FROM game_character_mission_info),(SELECT COUNT(*) FROM game_sp_skills),(SELECT COUNT(*) FROM game_titles),(SELECT COUNT(*) FROM game_micro_play_status),(SELECT COUNT(*) FROM game_skills)").fetch_one(p).await.unwrap()
}
async fn base(p: &PgPool) -> (i32, i32, i32, i32, i32, i32, i32, bool) {
    sqlx::query_as(
        "SELECT str,dex,int,will,luck,hp,stamina,dirty FROM game_character_info WHERE id=1",
    )
    .fetch_one(p)
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn ordered_prefix_single_winner_no_join_or_grants_and_no_replay() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let profile = profile("25");
    let catalog = catalog();
    let owner = owner();
    let (x, y) = tokio::join!(
        managers::initialize(&p, &owner, 1, &profile, &catalog),
        managers::initialize(&p, &owner, 1, &profile, &catalog)
    );
    assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
    let (winner, loser) = if x.is_ok() { (x, y) } else { (y, x) };
    let outcome = winner.unwrap();
    assert_eq!(outcome.cid(), 1);
    assert_eq!(*outcome.base().values(), [1, 2, 3, 4, 100, 5, 6]);
    assert!(matches!(loser, Err(ManagerError::Conflict)));
    assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
    assert_eq!(base(&p).await, (1, 2, 3, 4, 100, 5, 6, false));
    let slot: (i32, Option<Vec<u8>>) =
        sqlx::query_as("SELECT slot_id,skill_utf16 FROM game_sp_skills WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(slot, (0, None));
    assert!(matches!(
        managers::initialize(&p, &owner, 1, &profile, &catalog).await,
        Err(ManagerError::Conflict)
    ));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn ordered_prefix_preserves_prior_commits_on_ratio_stats_and_sp_failure() {
    for phase in 0..3 {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        if phase > 0 {
            sqlx::query("CREATE FUNCTION reject_manager_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic failure';END$$").execute(&p).await.unwrap();
            let target = if phase == 1 {
                "BEFORE UPDATE ON game_character_info"
            } else {
                "BEFORE INSERT ON game_sp_skills"
            };
            sqlx::query(&format!("CREATE TRIGGER reject_manager_write {target} FOR EACH ROW EXECUTE FUNCTION reject_manager_write()")).execute(&p).await.unwrap();
        }
        let profile = profile(if phase == 0 { "2147483648" } else { "25" });
        let catalog = catalog();
        let result = managers::initialize(&p, &owner(), 1, &profile, &catalog).await;
        match phase {
            0 => assert!(matches!(result, Err(ManagerError::Content(_)))),
            1 => assert!(matches!(result, Err(ManagerError::Stats(_)))),
            _ => assert!(matches!(result, Err(ManagerError::Skills(_)))),
        }
        assert_eq!(counts(&p).await, (1, 0, 0, 0, 0));
        assert_eq!(
            base(&p).await,
            if phase == 2 {
                (1, 2, 3, 4, 100, 5, 6, false)
            } else {
                (0, 0, 0, 0, 0, 0, 0, true)
            }
        );
        assert!(matches!(
            managers::initialize(&p, &owner(), 1, &profile, &catalog).await,
            Err(ManagerError::Conflict)
        ));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn ordered_prefix_refuses_catalog_source_and_preexisting_state_before_writes() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let profile = profile("25");
    let mut cat = catalog();
    cat.manifest.database_sha256 = "d".repeat(64);
    assert!(matches!(
        managers::initialize(&p, &owner(), 1, &profile, &cat).await,
        Err(ManagerError::Content(_))
    ));
    assert_eq!(counts(&p).await, (0, 0, 0, 0, 0));
    sqlx::query("UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton")
        .bind(vec![0u8; 32])
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        managers::initialize(&p, &owner(), 1, &profile, &catalog()).await,
        Err(ManagerError::Conflict)
    ));
    sqlx::query("UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton")
        .bind(profile.source_state().as_slice())
        .execute(&p)
        .await
        .unwrap();
    character_skills::initialize_sp_slot(&p, &owner(), 1)
        .await
        .unwrap();
    assert!(matches!(
        managers::initialize(&p, &owner(), 1, &profile, &catalog()).await,
        Err(ManagerError::Conflict)
    ));
    assert_eq!(counts(&p).await, (0, 1, 0, 0, 0));
    assert_eq!(base(&p).await, (0, 0, 0, 0, 0, 0, 0, true));
    cleanup_game_pool(a, p, s).await;
}
async fn setup(p: &PgPool) {
    provision_game_user(p, 31, "Owner", 900, 0).await.unwrap();
    sqlx::query("INSERT INTO game_lab_account_origins(uid,name_key,profile_fingerprint) SELECT id,name_key,$1 FROM game_users WHERE id=31").bind(vec![0u8;32]).execute(p).await.unwrap();
    sqlx::query("INSERT INTO game_my_settings(code_bytes,value_bytes)VALUES($1,$2)")
        .bind(b"FreeItem".as_slice())
        .bind(b"0".as_slice())
        .execute(p)
        .await
        .unwrap();
    let proof = common::proof(&common::content());
    character_creation_core::create_base(
        p,
        owner(),
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
    let bytes = serde_json::to_vec(&state_export()).unwrap();
    state::preserve(p, &state::read_snapshot(bytes.as_slice()).unwrap(), false)
        .await
        .unwrap();
    let bytes=serde_json::to_vec(&json!({"format_version":1,"source":"study_heroes.skill-state","sp_source_last_value":null,"skills":[],"sp_slots":[]})).unwrap();
    character_skills::preserve(
        p,
        &character_skills::read_snapshot(bytes.as_slice()).unwrap(),
        false,
    )
    .await
    .unwrap();
}

async fn costume_source(p: &PgPool, nonempty: bool) {
    use vindictus_server::infrastructure::character_costumes as costume;
    let rows = if nonempty {
        json!([
        {"row_id":"1","cid":"1","is_base":false,"costume_type":0,"costume_key":4,"costume_sn":null,"c1":null,"c2":2,"c3":null,"gloss":9,"effect":null},
        {"row_id":"2","cid":"1","is_base":true,"costume_type":0,"costume_key":4,"costume_sn":103,"c1":1,"c2":null,"c3":3,"gloss":null,"effect":-7},
        {"row_id":"3","cid":"1","is_base":true,"costume_type":3,"costume_key":2,"costume_sn":3,"c1":null,"c2":null,"c3":null,"gloss":null,"effect":null},
        {"row_id":"4","cid":"1","is_base":true,"costume_type":0,"costume_key":1,"costume_sn":-3,"c1":null,"c2":null,"c3":null,"gloss":null,"effect":null},
        {"row_id":"5","cid":"1","is_base":false,"costume_type":255,"costume_key":0,"costume_sn":7,"c1":null,"c2":null,"c3":null,"gloss":null,"effect":null}
        ])
    } else {
        json!([])
    };
    let bytes=serde_json::to_vec(&json!({"format_version":1,"source":"study_heroes.costume","source_last_value":if nonempty{json!("5")}else{Value::Null},"rows":rows})).unwrap();
    costume::preserve(p, &costume::read_snapshot(bytes.as_slice()).unwrap(), false)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn constructor_costume_loads_context_maps_and_overlay_without_save_or_sequence() {
    use vindictus_server::application::initial_costume::Context;
    for nonempty in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        costume_source(&p, nonempty).await;
        let prefix = managers::initialize(&p, &owner(), 1, &profile("25"), &catalog())
            .await
            .unwrap();
        let before:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY row_id),'[]'::jsonb) FROM game_costumes c").fetch_one(&p).await.unwrap();
        let seq: (i64, bool) =
            sqlx::query_as("SELECT last_value,is_called FROM game_costume_row_id")
                .fetch_one(&p)
                .await
                .unwrap();
        sqlx::query("CREATE FUNCTION reject_costume_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected constructor write';END$$").execute(&p).await.unwrap();
        for table in ["game_costumes", "game_costume_identity"] {
            sqlx::query(&format!("CREATE TRIGGER reject_costume_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_costume_write()")).execute(&p).await.unwrap();
        }
        let loaded = managers::load_costume(
            &p,
            &owner(),
            prefix,
            Context {
                cafe_type: i32::MIN,
                is_return: true,
            },
        )
        .await
        .unwrap();
        assert_eq!(loaded.cid(), 1);
        assert_eq!(*loaded.base().values(), [1, 2, 3, 4, 100, 5, 6]);
        let info = loaded.costume().info();
        assert_eq!(info.cafe_type, i32::MIN);
        assert!(info.is_return);
        assert_eq!(info.town_effect, 0);
        if nonempty {
            assert_eq!(
                loaded
                    .costume()
                    .base_rows()
                    .map(|r| (r.kind, r.key))
                    .collect::<Vec<_>>(),
                vec![(0, 4), (0, 1), (3, 2)]
            );
            assert_eq!(loaded.costume().item_rows().count(), 2);
            assert_eq!(
                info.costume_type_info.as_ref().unwrap(),
                &vec![(4, 103), (1, -3)]
            );
            assert_eq!(
                info.color_info.as_ref().unwrap(),
                &vec![(12, 1), (14, 3), (13, 2)]
            );
            assert_eq!(info.effect_info.as_ref().unwrap(), &vec![(4, -1), (1, -1)]);
        } else {
            assert_eq!(loaded.costume().base_rows().count(), 0);
            assert_eq!(loaded.costume().item_rows().count(), 0);
        }
        let after:Value=sqlx::query_scalar("SELECT COALESCE(jsonb_agg(to_jsonb(c) ORDER BY row_id),'[]'::jsonb) FROM game_costumes c").fetch_one(&p).await.unwrap();
        assert_eq!(before, after);
        assert_eq!(
            seq,
            sqlx::query_as::<_, (i64, bool)>(
                "SELECT last_value,is_called FROM game_costume_row_id"
            )
            .fetch_one(&p)
            .await
            .unwrap()
        );
        assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
        assert_eq!(base(&p).await, (1, 2, 3, 4, 100, 5, 6, false));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn constructor_costume_refuses_stale_source_uninitialized_and_changed_boundary() {
    use vindictus_server::application::initial_costume::Context;
    for phase in 0..4 {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        if phase != 2 {
            costume_source(&p, false).await;
        }
        let prefix = managers::initialize(&p, &owner(), 1, &profile("25"), &catalog())
            .await
            .unwrap();
        match phase {
            0 => {
                sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query(
                    "UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton",
                )
                .bind(vec![0u8; 32])
                .execute(&p)
                .await
                .unwrap();
            }
            3 => {
                sqlx::query("UPDATE game_character_info SET hp=hp+1 WHERE id=1")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            _ => {}
        }
        let result = managers::load_costume(
            &p,
            &owner(),
            prefix,
            Context {
                cafe_type: 0,
                is_return: false,
            },
        )
        .await;
        match phase {
            0 => assert!(matches!(result, Err(ManagerError::State(_)))),
            2 => assert!(matches!(result, Err(ManagerError::Costume(_)))),
            _ => assert!(matches!(result, Err(ManagerError::Conflict))),
        }
        assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn constructor_costume_refuses_ambiguous_and_excess_rows_preserving_prefix() {
    use vindictus_server::application::initial_costume::Context;
    for duplicate in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        costume_source(&p, false).await;
        let prefix = managers::initialize(&p, &owner(), 1, &profile("25"), &catalog())
            .await
            .unwrap();
        if duplicate {
            sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key) VALUES(1,1,true,0,0),(2,1,true,0,0)").execute(&p).await.unwrap();
        } else {
            sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key) SELECT n+1,1,true,200+n/256,n%256 FROM generate_series(0,1024) n").execute(&p).await.unwrap();
        }
        let result = managers::load_costume(
            &p,
            &owner(),
            prefix,
            Context {
                cafe_type: 0,
                is_return: false,
            },
        )
        .await;
        assert!(matches!(result, Err(ManagerError::Costume(_))));
        assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
        assert_eq!(base(&p).await, (1, 2, 3, 4, 100, 5, 6, false));
        cleanup_game_pool(a, p, s).await;
    }
}
