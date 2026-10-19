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
    setup_cid(p, 1).await
}
async fn setup_cid(p: &PgPool, cid: i64) {
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
            cid,
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

fn vocation_export() -> Value {
    json!({"format_version":1,"source":"study_heroes.vocation-state","source_last_value":"100","vocations":[{"cid":"1","vocation_class":-2,"vocation_level":0,"vocation_exp":i32::MIN,"last_transform_ticks":"3155378975999999999"}],"skills":[{"row_id":"2","cid":"1","skill_utf16":"4100","rank":i32::MAX},{"row_id":"9","cid":"1","skill_utf16":"4100","rank":1},{"row_id":"10","cid":"1","skill_utf16":"6100","rank":-1}]})
}
fn vocation_snapshot(v: &Value) -> vindictus_server::infrastructure::character_vocation::Snapshot {
    vindictus_server::infrastructure::character_vocation::read_snapshot(
        serde_json::to_vec(v).unwrap().as_slice(),
    )
    .unwrap()
}
async fn empty_vocation_source(p: &PgPool) {
    use vindictus_server::infrastructure::character_vocation as v;
    let e = json!({"format_version":1,"source":"study_heroes.vocation-state","source_last_value":null,"vocations":[],"skills":[]});
    v::preserve(p, &vocation_snapshot(&e), false).await.unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn vocation_preservation_compares_all_fields_freezes_source_and_never_reseeds_replay() {
    use vindictus_server::infrastructure::character_vocation::{self as v, VocationError};
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let e = vocation_export();
    let snapshot = vocation_snapshot(&e);
    let (x, y) = tokio::join!(
        v::preserve(&p, &snapshot, false),
        v::preserve(&p, &snapshot, false)
    );
    assert_eq!(x.unwrap().inserted + y.unwrap().inserted, 4);
    assert_eq!(v::preserve(&p, &snapshot, true).await.unwrap().verified, 4);
    let actual:(i32,i32,i32,Option<i64>)=sqlx::query_as("SELECT vocation_class,vocation_level,vocation_exp,last_transform_ticks FROM game_vocations WHERE cid=1").fetch_one(&p).await.unwrap();
    assert_eq!(actual, (-2, 0, i32::MIN, Some(3155378975999999999)));
    let actual: Vec<(i64, i64, Vec<u8>, i32)> = sqlx::query_as(
        "SELECT row_id,cid,skill_utf16,rank FROM game_vocation_skills ORDER BY row_id",
    )
    .fetch_all(&p)
    .await
    .unwrap();
    assert_eq!(
        actual,
        vec![
            (2, 1, vec![65, 0], i32::MAX),
            (9, 1, vec![65, 0], 1),
            (10, 1, vec![97, 0], -1)
        ]
    );
    let first: i64 = sqlx::query_scalar("SELECT nextval('game_vocation_skill_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(first, 101);
    v::preserve(&p, &snapshot, false).await.unwrap();
    let second: i64 = sqlx::query_scalar("SELECT nextval('game_vocation_skill_row_id')")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(second, 102);
    let mut changed = e.clone();
    changed["vocations"][0]["vocation_exp"] = json!(0);
    assert!(matches!(
        v::preserve(&p, &vocation_snapshot(&changed), false).await,
        Err(VocationError::Conflict)
    ));
    sqlx::query("UPDATE game_vocation_skills SET rank=7 WHERE row_id=9")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        v::preserve(&p, &snapshot, true).await,
        Err(VocationError::Conflict)
    ));
    let rank: i32 = sqlx::query_scalar("SELECT rank FROM game_vocation_skills WHERE row_id=9")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(rank, 7);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn vocation_preservation_late_conflict_rolls_back_rows_marker_and_sequence() {
    use vindictus_server::infrastructure::character_vocation::{self as v, VocationError};
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("INSERT INTO game_vocations VALUES(1,-2,0,-2147483648,3155378975999999999)")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_vocation_skills VALUES(9,1,$1,7)")
        .bind(vec![65u8, 0])
        .execute(&p)
        .await
        .unwrap();
    let snapshot = vocation_snapshot(&vocation_export());
    assert!(matches!(
        v::preserve(&p, &snapshot, false).await,
        Err(VocationError::Conflict)
    ));
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_vocation_skills")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 1);
    let state:(bool,Option<i64>,Option<Vec<u8>>)=sqlx::query_as("SELECT initialized,source_last_value,source_sha256 FROM game_vocation_preservation WHERE singleton").fetch_one(&p).await.unwrap();
    assert_eq!(state, (false, None, None));
    let seq: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_vocation_skill_row_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(seq, (1, false));
    assert!(
        sqlx::query("INSERT INTO game_vocation_skills VALUES(99,999,$1,0)")
            .bind(vec![65u8, 0])
            .execute(&p)
            .await
            .is_err()
    );
    sqlx::query("DELETE FROM game_vocation_skills")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("INSERT INTO game_vocation_skills VALUES(99,1,$1,0)")
        .bind(vec![65u8, 0])
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        v::preserve(&p, &snapshot, false).await,
        Err(VocationError::Conflict)
    ));
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM game_vocation_skills")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 1);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn vocation_constructor_after_costume_proves_none_without_write_or_identity() {
    use vindictus_server::application::initial_costume::Context;
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    costume_source(&p, false).await;
    empty_vocation_source(&p).await;
    let prefix = managers::initialize(&p, &owner(), 1, &profile("25"), &catalog())
        .await
        .unwrap();
    let prior = managers::load_costume(
        &p,
        &owner(),
        prefix,
        Context {
            cafe_type: 0,
            is_return: true,
        },
    )
    .await
    .unwrap();
    sqlx::query("CREATE FUNCTION reject_vocation_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected vocation constructor write';END$$").execute(&p).await.unwrap();
    for table in [
        "game_vocations",
        "game_vocation_skills",
        "game_vocation_preservation",
    ] {
        sqlx::query(&format!("CREATE TRIGGER reject_vocation_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_vocation_write()")).execute(&p).await.unwrap();
    }
    let out = managers::load_vocation(&p, &owner(), prior).await.unwrap();
    assert_eq!(out.cid(), 1);
    assert_eq!(*out.base().values(), [1, 2, 3, 4, 100, 5, 6]);
    assert!(out.costume().info().is_return);
    let v = out.vocation();
    assert!(v.details().is_none());
    assert_eq!(v.skills().count(), 0);
    assert!(v.changed_skills().is_empty());
    assert_eq!(v.used_points(), 0);
    assert_eq!(v.duplicates(), 0);
    assert!(!v.vocation_changed());
    assert_eq!(v.transform_level(), 0);
    let seq: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_vocation_skill_row_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(seq, (1, false));
    assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn vocation_constructor_refuses_uninitialized_existing_stale_costume_and_revision() {
    use vindictus_server::{
        application::initial_costume::Context, infrastructure::character_vocation as v,
    };
    for phase in 0..4 {
        let (a, p, s) = isolated_game_pool().await;
        setup(&p).await;
        costume_source(&p, false).await;
        if phase == 1 {
            v::preserve(&p, &vocation_snapshot(&vocation_export()), false)
                .await
                .unwrap();
        } else if phase != 0 {
            empty_vocation_source(&p).await;
        }
        let prefix = managers::initialize(&p, &owner(), 1, &profile("25"), &catalog())
            .await
            .unwrap();
        let prior = managers::load_costume(
            &p,
            &owner(),
            prefix,
            Context {
                cafe_type: 0,
                is_return: false,
            },
        )
        .await
        .unwrap();
        if phase == 2 {
            sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn)VALUES(1,1,true,0,0,99)").execute(&p).await.unwrap();
        }
        if phase == 3 {
            sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                .execute(&p)
                .await
                .unwrap();
        }
        let result = managers::load_vocation(&p, &owner(), prior).await;
        match phase {
            0 | 1 => assert!(matches!(result, Err(ManagerError::Vocation(_)))),
            2 => assert!(matches!(result, Err(ManagerError::Conflict))),
            _ => assert!(matches!(result, Err(ManagerError::State(_)))),
        }
        assert_eq!(counts(&p).await, (1, 1, 0, 0, 0));
        cleanup_game_pool(a, p, s).await;
    }
}

async fn social_prior(p: &PgPool) -> managers::VocatedManagers {
    use vindictus_server::application::initial_costume::Context;
    setup(p).await;
    costume_source(p, false).await;
    empty_vocation_source(p).await;
    let prefix = managers::initialize(p, &owner(), 1, &profile("25"), &catalog())
        .await
        .unwrap();
    let costume = managers::load_costume(
        p,
        &owner(),
        prefix,
        Context {
            cafe_type: 0,
            is_return: true,
        },
    )
    .await
    .unwrap();
    managers::load_vocation(p, &owner(), costume).await.unwrap()
}
async fn social_sequences(p: &PgPool) -> Vec<(String, Option<i64>)> {
    sqlx::query_as("SELECT sequencename::text,last_value FROM pg_sequences WHERE schemaname=current_schema() ORDER BY sequencename").fetch_all(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn social_constructor_binds_shared_context_without_writes_ids_or_guild_query() {
    let (a, p, s) = isolated_game_pool().await;
    let prior = social_prior(&p).await;
    let before = counts(&p).await;
    let sequences = social_sequences(&p).await;
    sqlx::query("CREATE FUNCTION reject_social_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected social constructor write';END$$").execute(&p).await.unwrap();
    let tables:Vec<String>=sqlx::query_scalar("SELECT tablename::text FROM pg_tables WHERE schemaname=current_schema() AND tablename LIKE 'game_%'").fetch_all(&p).await.unwrap();
    for table in tables {
        assert!(
            table
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        );
        sqlx::query(&format!("CREATE TRIGGER reject_social_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_social_write()")).execute(&p).await.unwrap();
    }
    // Guild/Whisper service tables are intentionally absent: the admitted original
    // constructors do not query them. This does not infer empty persisted state.
    let out = managers::load_social(&p, &owner(), prior, &profile("25"), &catalog())
        .await
        .unwrap();
    assert_eq!(out.cid(), 1);
    assert_eq!(*out.base().values(), [1, 2, 3, 4, 100, 5, 6]);
    assert!(out.costume().info().is_return);
    assert!(out.vocation().details().is_none());
    let social = out.social();
    assert_eq!(social.whisper().uid(), 31);
    assert_eq!(social.whisper().cid(), 1);
    assert_eq!(social.guild().uid(), 31);
    assert_eq!(social.guild().cid(), 1);
    assert!(social.shares_player_context());
    assert!(social.whisper().shares_character_player_context());
    assert!(!social.guild().info_loaded());
    assert_eq!(social.guild().pending_procedure_count(), 0);
    assert_eq!(counts(&p).await, before);
    assert_eq!(social_sequences(&p).await, sequences);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn social_constructor_refuses_stale_prior_owner_source_profile_and_catalog() {
    for phase in 0..8 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = social_prior(&p).await;
        let before = counts(&p).await;
        let sequences = social_sequences(&p).await;
        let mut key = owner();
        let mut cat = catalog();
        let mut prof = profile("25");
        match phase {
            0 => {
                sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn)VALUES(1,1,true,0,0,99)").execute(&p).await.unwrap();
            }
            2 => {
                sqlx::query("INSERT INTO game_vocations VALUES(1,0,0,0,NULL)")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            3 => {
                sqlx::query(
                    "UPDATE game_vocation_preservation SET initialized=false WHERE singleton",
                )
                .execute(&p)
                .await
                .unwrap();
            }
            4 => {
                sqlx::query(
                    "UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton",
                )
                .bind(vec![7u8; 32])
                .execute(&p)
                .await
                .unwrap();
            }
            5 => cat.manifest.config_sha256 = "c".repeat(64),
            6 => {
                let mut raw = profile_fixture::synthetic();
                raw["state_sha256"] = json!("d".repeat(64));
                prof = profile_fixture::read(&raw).unwrap();
            }
            _ => key.uid = 32,
        }
        let out = managers::load_social(&p, &key, prior, &prof, &cat).await;
        match phase {
            0 | 7 => assert!(matches!(out, Err(ManagerError::State(_)))),
            2 | 3 => assert!(matches!(out, Err(ManagerError::Vocation(_)))),
            5 => assert!(matches!(out, Err(ManagerError::Content(_)))),
            _ => assert!(matches!(out, Err(ManagerError::Conflict))),
        }
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, sequences);
        cleanup_game_pool(a, p, s).await;
    }
}

async fn status_prior(p: &PgPool, initialized: bool) -> managers::SocialManagers {
    let prior = social_prior(p).await;
    if initialized {
        use vindictus_server::infrastructure::character_status_effects as db;
        let raw=serde_json::json!({"format_version":1,"source":"study_heroes.status-effects","source_last_value":"100","effects":[]}).to_string();
        let source = db::read_snapshot(raw.as_bytes()).unwrap();
        db::preserve(p, &source, false).await.unwrap();
    }
    managers::load_social(p, &owner(), prior, &profile("25"), &catalog())
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_constructor_runs_clock_save_then_campfire_without_writes_or_ids() {
    use vindictus_server::domain::initial_status::Profile;
    let (a, p, s) = isolated_game_pool().await;
    let prior = status_prior(&p, true).await;
    let before = counts(&p).await;
    let sequences = social_sequences(&p).await;
    sqlx::query("CREATE FUNCTION reject_status_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected constructor write';END$$").execute(&p).await.unwrap();
    let tables:Vec<String>=sqlx::query_scalar("SELECT tablename::text FROM pg_tables WHERE schemaname=current_schema() AND tablename LIKE 'game_%'").fetch_all(&p).await.unwrap();
    for table in tables {
        assert!(
            table
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        );
        sqlx::query(&format!("CREATE TRIGGER reject_status_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_status_write()")).execute(&p).await.unwrap();
    }
    let mut observations = Profile::default();
    let mut calls = 0;
    let out = managers::load_status(
        &p,
        &owner(),
        prior,
        || {
            calls += 1;
            vindictus_server::infrastructure::lab_clock::utc_now()
        },
        &mut observations,
    )
    .await
    .unwrap();
    assert_eq!(calls, 1);
    assert_eq!(observations.calls(), 1);
    assert_eq!(out.prior().cid(), 1);
    let loaded = out.loaded();
    assert!(loaded.matches_social(out.prior().social()));
    assert!(loaded.shares_character_context());
    assert_eq!(loaded.status().cid(), 1);
    assert_eq!(loaded.status().uid(), 31);
    assert_eq!(loaded.status().type_count(), 0);
    assert_eq!(loaded.status().tag_count(), 0);
    assert!(loaded.status().save_succeeded());
    assert!(!loaded.status().need_db_submit());
    assert!(!loaded.status().need_update_stat());
    assert_eq!(loaded.status().stats().entries().count(), 25);
    assert!(loaded.status().stats().entries().all(|(_, v)| v == 0));
    assert_eq!(loaded.campfire().status(), "");
    assert_eq!(counts(&p).await, before);
    assert_eq!(social_sequences(&p).await, sequences);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn status_constructor_refuses_invalid_boundaries_before_clock_and_preserves_clock_failure() {
    use vindictus_server::{
        domain::initial_status::Profile, infrastructure::lab_clock::ClockError,
    };
    for phase in 0..8 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = status_prior(&p, phase != 0).await;
        let mut key = owner();
        match phase {
            1 => {
                sqlx::query("INSERT INTO game_status_effects VALUES(1,1,$1,0,0,0,0)")
                    .bind(vec![65u8, 0])
                    .execute(&p)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            3 => {
                sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn)VALUES(1,1,true,0,0,99)").execute(&p).await.unwrap();
            }
            4 => {
                sqlx::query("INSERT INTO game_vocations VALUES(1,0,0,0,NULL)")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            5 => {
                sqlx::query(
                    "UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton",
                )
                .bind(vec![7u8; 32])
                .execute(&p)
                .await
                .unwrap();
            }
            6 => key.uid = 32,
            _ => {}
        }
        let before = counts(&p).await;
        let sequences = social_sequences(&p).await;
        let mut observations = Profile::default();
        let mut calls = 0;
        let result = managers::load_status(
            &p,
            &key,
            prior,
            || {
                calls += 1;
                Err(ClockError::Range)
            },
            &mut observations,
        )
        .await;
        assert!(result.is_err());
        if phase == 7 {
            assert!(matches!(result, Err(ManagerError::Clock(_))));
            assert_eq!(calls, 1);
            assert_eq!(observations.calls(), 1)
        } else {
            assert_eq!(calls, 0);
            assert_eq!(observations.calls(), 0)
        }
        if phase == 0 || phase == 1 {
            assert!(matches!(result, Err(ManagerError::Status(_))));
        }
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, sequences);
        if phase == 1 {
            let row:(i64,Vec<u8>,i32,Option<i32>,Option<i64>,Option<i16>)=sqlx::query_as("SELECT cid,type_utf16,level,remain_time,expire_ticks,remain_micro FROM game_status_effects WHERE row_id=1").fetch_one(&p).await.unwrap();
            assert_eq!(row, (1, vec![65u8, 0], 0, Some(0), Some(0), Some(0)));
        }
        cleanup_game_pool(a, p, s).await;
    }
}

async fn attendance_prior(p: &PgPool) -> managers::StatusManagers {
    let prior = status_prior(p, true).await;
    let mut observations = vindictus_server::domain::initial_status::Profile::default();
    managers::load_status(
        p,
        &owner(),
        prior,
        vindictus_server::infrastructure::lab_clock::utc_now,
        &mut observations,
    )
    .await
    .unwrap()
}
fn attendance_profile(version: &str, enabled: bool) -> Profile {
    let mut raw = profile_fixture::synthetic();
    raw["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
    ));
    for r in raw["features"].as_array_mut().unwrap() {
        if r["key"] == "AttendanceEvent_Newbie" {
            r["value"] = json!(version);
        }
        if r["key"] == "Attendance_Newbie" {
            r["value"] = json!(if enabled {
                "synthetic-on"
            } else {
                "synthetic-off"
            });
        }
    }
    profile_fixture::read(&raw).unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn attendance_constructor_expired_first_user_without_dml_or_identity() {
    let (a, p, s) = isolated_game_pool().await;
    let prior = attendance_prior(&p).await;
    sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
        .execute(&p)
        .await
        .unwrap();
    let before = counts(&p).await;
    let sequences = social_sequences(&p).await;
    sqlx::query("CREATE FUNCTION reject_attendance_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected attendance constructor write';END$$").execute(&p).await.unwrap();
    let tables:Vec<String>=sqlx::query_scalar("SELECT tablename::text FROM pg_tables WHERE schemaname=current_schema() AND tablename LIKE 'game_%'").fetch_all(&p).await.unwrap();
    for table in tables {
        assert!(
            table
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        );
        sqlx::query(&format!("CREATE TRIGGER reject_attendance_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_attendance_write()")).execute(&p).await.unwrap();
    }
    let mut calls = 0;
    let out = managers::load_attendance(&p, &owner(), prior, &profile("25"), &catalog(), || {
        calls += 1;
        vindictus_server::infrastructure::lab_clock::utc_now()
    })
    .await
    .unwrap();
    assert_eq!(calls, 2);
    assert_eq!(out.prior().prior().cid(), 1);
    let loaded = out.loaded();
    assert!(loaded.matches_social(out.prior().prior().social()));
    assert_eq!(loaded.instance_count(), 1);
    assert_eq!(
        loaded.outcome(),
        vindictus_server::domain::initial_attendance::Outcome::NoNewbie
    );
    let all = loaded.all_user_state();
    assert_eq!(all.uid(), 31);
    assert_eq!(all.cid(), 1);
    assert_eq!(all.event_version(), -1);
    assert_eq!(
        (
            all.date_from_ticks(),
            all.date_until_ticks(),
            all.last_feature_error()
        ),
        (0, 0, 0)
    );
    assert!(!all.db_destroyed_callback_present());
    assert!(all.context_matches_parent());
    assert_eq!((all.attendance_count(), all.bonus_count()), (0, 0));
    let user: (Option<i64>, bool) = sqlx::query_as(
        "SELECT last_login_cid,prologue_played FROM game_user_info WHERE nexon_sn=31",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(user, (None, true));
    assert_eq!(counts(&p).await, before);
    assert_eq!(social_sequences(&p).await, sequences);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn attendance_constructor_refuses_stale_prior_before_utc() {
    for phase in 0..8 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = attendance_prior(&p).await;
        sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
            .execute(&p)
            .await
            .unwrap();
        let mut key = owner();
        let mut cat = catalog();
        let mut prof = profile("25");
        match phase {
            0 => {
                sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("INSERT INTO game_status_effects VALUES(1,1,$1,0,0,0,0)")
                    .bind(vec![65u8, 0])
                    .execute(&p)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn)VALUES(1,1,true,0,0,99)").execute(&p).await.unwrap();
            }
            3 => {
                sqlx::query("INSERT INTO game_vocations VALUES(1,0,0,0,NULL)")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            4 => {
                sqlx::query(
                    "UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton",
                )
                .bind(vec![7u8; 32])
                .execute(&p)
                .await
                .unwrap();
            }
            5 => cat.manifest.config_sha256 = "c".repeat(64),
            6 => {
                let mut raw = profile_fixture::synthetic();
                raw["state_sha256"] = json!("c".repeat(64));
                prof = profile_fixture::read(&raw).unwrap();
            }
            _ => key.uid = 32,
        }
        let before = counts(&p).await;
        let sequences = social_sequences(&p).await;
        let mut calls = 0;
        assert!(
            managers::load_attendance(&p, &key, prior, &prof, &cat, || {
                calls += 1;
                vindictus_server::infrastructure::lab_clock::utc_now()
            })
            .await
            .is_err()
        );
        assert_eq!(calls, 0);
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, sequences);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn attendance_constructor_preserves_first_query_window_and_caught_failure_order() {
    use vindictus_server::{
        domain::{initial_attendance::Outcome, legacy_user_data::SqlDateTime2},
        infrastructure::lab_clock::ClockError,
    };
    for phase in 0..9 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = attendance_prior(&p).await;
        if phase != 0 && phase != 7 {
            sqlx::query("INSERT INTO game_user_info VALUES(31,$1,true)")
                .bind(if phase == 1 { Some(42i64) } else { None })
                .execute(&p)
                .await
                .unwrap();
        }
        let prof = attendance_profile(if phase == 8 { "2147483648" } else { "7" }, phase != 7);
        let before = counts(&p).await;
        let sequences = social_sequences(&p).await;
        let mut calls = 0;
        let result = managers::load_attendance(&p, &owner(), prior, &prof, &catalog(), || {
            calls += 1;
            if (phase == 5 && calls == 1) || (phase == 6 && calls == 2) {
                Err(ClockError::Range)
            } else {
                Ok(SqlDateTime2::from_ticks(match phase {
                    3 => 50,
                    4 => 250,
                    _ => 150,
                })
                .unwrap())
            }
        })
        .await;
        match phase {
            0 | 1 => {
                assert!(matches!(result, Err(ManagerError::NewbieAttendance(vindictus_server::infrastructure::newbie_attendance::AttendanceError::Reference))));
                assert_eq!(calls, 0);
            }
            2 => {
                assert!(matches!(result, Err(ManagerError::Attendance(_))));
                assert_eq!(calls, 2);
            }
            _ => {
                let out = result.unwrap();
                let expected = match phase {
                    5 | 6 => Outcome::CaughtClockError,
                    7 => Outcome::Disabled,
                    8 => Outcome::CaughtIntegerOverflow,
                    _ => Outcome::NoNewbie,
                };
                assert_eq!(out.loaded().outcome(), expected);
                assert_eq!(
                    calls,
                    match phase {
                        3 | 5 => 1,
                        4 | 6 => 2,
                        _ => 0,
                    }
                );
            }
        }
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, sequences);
        cleanup_game_pool(a, p, s).await;
    }
}

fn events_profile() -> Profile {
    profile("25")
}
async fn events_prior(p: &PgPool, bootstrap: bool) -> managers::AttendanceManagers {
    let prior = attendance_prior(p).await;
    sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
        .execute(p)
        .await
        .unwrap();
    if bootstrap {
        use vindictus_server::infrastructure::item_journals as j;
        let source=j::read_snapshot(br#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":"611635799","character_last":"4419699"}"#.as_slice()).unwrap();
        j::preserve(p, &source, false).await.unwrap();
    }
    managers::load_attendance(
        p,
        &owner(),
        prior,
        &events_profile(),
        &catalog(),
        vindictus_server::infrastructure::lab_clock::utc_now,
    )
    .await
    .unwrap()
}
type EventJournalRow = (i64, Vec<u8>, i64, Option<Vec<u8>>);
async fn event_journals(p: &PgPool) -> Vec<EventJournalRow> {
    sqlx::query_as("SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal01 UNION ALL SELECT id,category_utf16,time_ticks,legacy_last_update FROM game_item_journal02 ORDER BY id").fetch_all(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn events_constructor_writes_only_independent_marble_journal_and_character_counter() {
    let (a, p, s) = isolated_game_pool().await;
    let prior = events_prior(&p, true).await;
    let before = counts(&p).await;
    let sequences = social_sequences(&p).await;
    sqlx::query("CREATE FUNCTION reject_event_write()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'unexpected event constructor write';END$$").execute(&p).await.unwrap();
    let tables:Vec<String>=sqlx::query_scalar("SELECT tablename::text FROM pg_tables WHERE schemaname=current_schema() AND tablename LIKE 'game_%' AND tablename NOT IN('game_item_journal01','game_item_journal02')").fetch_all(&p).await.unwrap();
    for table in tables {
        assert!(
            table
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_')
        );
        sqlx::query(&format!("CREATE TRIGGER reject_event_write BEFORE INSERT OR UPDATE OR DELETE ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_event_write()")).execute(&p).await.unwrap();
    }
    let out = managers::load_events(&p, &owner(), prior, &events_profile(), &catalog())
        .await
        .unwrap();
    let events = out.loaded();
    assert!(events.matches_social(out.prior().prior().prior().social()));
    assert!(events.shares_parent());
    assert!(events.contexts_distinct());
    assert_eq!(events.roulette().id(), -1);
    assert_eq!(events.roulette().version(), -1);
    assert!(events.roulette().initially_expired());
    assert_eq!(events.marble().journal_id(), 4419700);
    assert_eq!(events.marble().chance_face(), -1);
    assert!(!events.marble().initialized());
    let rows = event_journals(&p).await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, 4419700);
    assert_eq!(
        rows[0].1,
        "MarbleLog"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    );
    assert!(rows[0].2 > 0);
    assert!(rows[0].3.is_none());
    let item: (i64, bool) = sqlx::query_as("SELECT last_value,is_called FROM game_item_journal_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(item, (611635800, false));
    let character: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_character_journal_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(character, (4419700, true));
    let after = social_sequences(&p).await;
    assert_eq!(before, counts(&p).await);
    assert_eq!(
        sequences
            .into_iter()
            .filter(|(n, _)| n != "game_character_journal_id")
            .collect::<Vec<_>>(),
        after
            .into_iter()
            .filter(|(n, _)| n != "game_character_journal_id")
            .collect::<Vec<_>>()
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn events_constructor_keeps_suppressed_and_failed_journal_sentinel_without_reclaiming_id() {
    for phase in 0..3 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = events_prior(&p, phase != 0).await;
        if phase == 1 {
            sqlx::query("CREATE FUNCTION reject_marble_journal()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'injected journal insert failure';END$$").execute(&p).await.unwrap();
            for table in ["game_item_journal01", "game_item_journal02"] {
                sqlx::query(&format!("CREATE TRIGGER reject_marble_journal BEFORE INSERT ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_marble_journal()")).execute(&p).await.unwrap();
            }
        }
        if phase == 2 {
            sqlx::query("SELECT setval('game_character_journal_id',9223372036854775807,true)")
                .execute(&p)
                .await
                .unwrap();
        }
        let before = counts(&p).await;
        let id = if phase == 0 {
            // The normal profile forbids DisableLogging=true; test suppression
            // at the source helper without widening constructor admission.
            drop(prior);
            vindictus_server::infrastructure::item_journals::add_marble_journal(&p, true).await
        } else {
            let out = managers::load_events(&p, &owner(), prior, &events_profile(), &catalog())
                .await
                .unwrap();
            assert_eq!(out.loaded().marble().chance_face(), -1);
            out.loaded().marble().journal_id()
        };
        assert_eq!(id, -1);
        assert_eq!(event_journals(&p).await.len(), 0);
        assert_eq!(counts(&p).await, before);
        let seq: (i64, bool) =
            sqlx::query_as("SELECT last_value,is_called FROM game_character_journal_id")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(
            seq,
            match phase {
                0 => (4419625, false),
                1 => (4419700, true),
                _ => (i64::MAX, true),
            }
        );
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn events_constructor_refuses_stale_prior_before_journal_or_identity() {
    for phase in 0..12 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = events_prior(&p, phase != 8).await;
        let mut key = owner();
        let mut prof = events_profile();
        let mut cat = catalog();
        match phase {
            0 => {
                sqlx::query("UPDATE game_users SET name=name WHERE id=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            1 => {
                sqlx::query("INSERT INTO game_status_effects VALUES(1,1,$1,0,0,0,0)")
                    .bind(vec![65u8, 0])
                    .execute(&p)
                    .await
                    .unwrap();
            }
            2 => {
                sqlx::query("INSERT INTO game_costumes(row_id,cid,is_base,costume_type,costume_key,costume_sn)VALUES(1,1,true,0,0,99)").execute(&p).await.unwrap();
            }
            3 => {
                sqlx::query("INSERT INTO game_vocations VALUES(1,0,0,0,NULL)")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            4 => {
                sqlx::query("UPDATE game_user_info SET last_login_cid=42 WHERE nexon_sn=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            5 => {
                sqlx::query("DELETE FROM game_user_info WHERE nexon_sn=31")
                    .execute(&p)
                    .await
                    .unwrap();
            }
            6 => prof = profile("26"),
            7 => cat.manifest.config_sha256 = "c".repeat(64),
            9 => {
                sqlx::query(
                    "UPDATE game_character_state_preservation SET source_sha256=$1 WHERE singleton",
                )
                .bind(vec![7u8; 32])
                .execute(&p)
                .await
                .unwrap();
            }
            10 => key.uid = 32,
            11 => {
                sqlx::query(
                    "UPDATE game_status_effect_preservation SET initialized=false WHERE singleton",
                )
                .execute(&p)
                .await
                .unwrap();
            }
            _ => {}
        }
        let before = counts(&p).await;
        let sequences = social_sequences(&p).await;
        assert!(
            managers::load_events(&p, &key, prior, &prof, &cat)
                .await
                .is_err()
        );
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, sequences);
        assert!(event_journals(&p).await.is_empty());
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn events_marble_journal_survives_independent_owner_rollback_without_replay_reseed() {
    use vindictus_server::infrastructure::item_journals as j;
    let (a, p, s) = isolated_game_pool().await;
    let prior = events_prior(&p, true).await;
    let mut tx = p.begin().await.unwrap();
    let uid: i32 = sqlx::query_scalar("SELECT id FROM game_users WHERE id=31 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let cid: i64 = sqlx::query_scalar("SELECT id FROM game_characters WHERE id=1 FOR UPDATE")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!((uid, cid), (31, 1));
    let id = j::add_marble_journal(&p, false).await;
    assert_eq!(id, 4419700);
    tx.rollback().await.unwrap();
    drop(prior);
    assert_eq!(event_journals(&p).await.len(), 1);
    let source=j::read_snapshot(br#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":"611635799","character_last":"4419699"}"#.as_slice()).unwrap();
    j::preserve(&p, &source, true).await.unwrap();
    let seq: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM game_character_journal_id")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(seq, (4419700, true));
    assert!(matches!(
        managers::initialize(&p, &owner(), 1, &events_profile(), &catalog()).await,
        Err(ManagerError::Conflict)
    ));
    cleanup_game_pool(a, p, s).await;
}

async fn captcha_prior(p: &PgPool) -> managers::EventManagers {
    let prior = events_prior(p, true).await;
    managers::load_events(p, &owner(), prior, &events_profile(), &catalog())
        .await
        .unwrap()
}
async fn seed_native_captcha(
    p: &PgPool,
    prior: &managers::EventManagers,
    source: vindictus_server::domain::initial_captcha::Source,
) {
    use vindictus_server::{
        domain::{initial_captcha::Loaded, legacy_user_data::SqlDateTime2},
        infrastructure::user_captcha as db,
    };
    let now = source
        .last_failure
        .map(|d| d.framework_date_time().unwrap())
        .unwrap_or(SqlDateTime2::from_ticks(0).unwrap());
    let state = Loaded::construct(prior.prior().prior().prior().social(), Some(source), || {
        Ok(now)
    })
    .unwrap();
    assert_eq!(
        db::save_empty_logout(p, &state).await,
        db::LogoutSave::Saved
    );
}
async fn captcha_bootstrap(p: &PgPool) {
    use vindictus_server::infrastructure::user_captcha as db;
    let snap = db::read_snapshot(
        br#"{"format_version":1,"source":"study_heroes.user-captcha","rows":[]}"#.as_slice(),
    )
    .unwrap();
    db::preserve(p, &snap, false).await.unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_constructor_absent_consumes_opaque_events_without_clock_dml_or_new_journal() {
    let (a, p, s) = isolated_game_pool().await;
    let prior = captcha_prior(&p).await;
    captcha_bootstrap(&p).await;
    let before = counts(&p).await;
    let seq = social_sequences(&p).await;
    let journals = event_journals(&p).await;
    let loaded = managers::load_captcha(&p, &owner(), prior, &events_profile(), &catalog(), || {
        panic!("absent row must not read clock")
    })
    .await
    .unwrap();
    assert_eq!(
        (
            loaded.loaded().uid(),
            loaded.loaded().cid(),
            loaded.loaded().pool_count()
        ),
        (31, 1, 0)
    );
    assert_eq!(counts(&p).await, before);
    assert_eq!(social_sequences(&p).await, seq);
    assert_eq!(event_journals(&p).await, journals);
    assert_eq!(loaded.prior().loaded().marble().journal_id(), 4419700);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_nullable_day_reset_and_independent_empty_logout_save_survive_owner_rollback() {
    use vindictus_server::infrastructure::user_captcha as db;
    for day in [0_i64, 1] {
        let (a, p, s) = isolated_game_pool().await;
        let prior = captcha_prior(&p).await;
        captcha_bootstrap(&p).await;
        seed_native_captcha(
            &p,
            &prior,
            vindictus_server::domain::initial_captcha::Source {
                punished: None,
                failure: Some(255),
                last_failure: Some(
                    vindictus_server::domain::legacy_user_data::SqlDateTime::from_units300(-1)
                        .unwrap(),
                ),
                last_succeed: Some(
                    vindictus_server::domain::legacy_user_data::SqlDateTime::from_units300(1)
                        .unwrap(),
                ),
                condition: Some(255),
            },
        )
        .await;
        let mut clocks = 0;
        let loaded =
            managers::load_captcha(&p, &owner(), prior, &events_profile(), &catalog(), || {
                clocks += 1;
                Ok(
                    vindictus_server::domain::legacy_user_data::SqlDateTime2::from_ticks(
                        599266080000000000 - 864000000000 + day * 864000000000,
                    )
                    .unwrap(),
                )
            })
            .await
            .unwrap();
        assert_eq!(clocks, 1);
        assert_eq!(loaded.loaded().failure(), if day == 0 { 255 } else { 0 });
        assert!(loaded.loaded().force());
        let before_save_sequence = social_sequences(&p).await;
        let mut tx = p.begin().await.unwrap();
        sqlx::query("SELECT id FROM game_users WHERE id=31 FOR UPDATE")
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(
            db::save_empty_logout(&p, loaded.loaded()).await,
            db::LogoutSave::Saved
        );
        tx.rollback().await.unwrap();
        let actual:CaptchaStored=sqlx::query_as("SELECT punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha WHERE uid=31").fetch_one(&p).await.unwrap();
        assert_eq!(
            actual,
            (
                Some(0),
                Some(if day == 0 { 255 } else { 0 }),
                Some(-1),
                Some(1),
                Some(1)
            )
        );
        assert_eq!(event_journals(&p).await.len(), 1);
        assert_eq!(
            db::save_empty_logout(&p, loaded.loaded()).await,
            db::LogoutSave::Saved
        );
        assert_eq!(social_sequences(&p).await, before_save_sequence);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_failures_keep_marble_journal_and_refuse_stale_prior_before_clock() {
    use vindictus_server::domain::initial_captcha::ConstructorError;
    for mode in 0..7 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = captcha_prior(&p).await;
        if mode != 0 {
            captcha_bootstrap(&p).await;
        }
        if mode == 1 {
            seed_native_captcha(
                &p,
                &prior,
                vindictus_server::domain::initial_captcha::Source {
                    punished: Some(0),
                    failure: Some(1),
                    last_failure: Some(
                        vindictus_server::domain::legacy_user_data::SqlDateTime::from_units300(0)
                            .unwrap(),
                    ),
                    last_succeed: None,
                    condition: Some(0),
                },
            )
            .await;
        }
        if mode == 2 {
            sqlx::query("UPDATE game_user_info SET last_login_cid=1 WHERE nexon_sn=31")
                .execute(&p)
                .await
                .unwrap();
        }
        if mode == 3 {
            sqlx::query("UPDATE game_character_info SET str=str+1 WHERE id=1")
                .execute(&p)
                .await
                .unwrap();
        }
        if mode == 4 {
            sqlx::query("UPDATE game_users SET service_id=service_id+1 WHERE id=31")
                .execute(&p)
                .await
                .unwrap();
        }
        if mode == 6 {
            sqlx::query("DROP TABLE game_user_captcha")
                .execute(&p)
                .await
                .unwrap();
        }
        let prof = if mode == 5 {
            profile("26")
        } else {
            events_profile()
        };
        let before = counts(&p).await;
        let seq = social_sequences(&p).await;
        let mut clocks = 0;
        let result = managers::load_captcha(&p, &owner(), prior, &prof, &catalog(), || {
            clocks += 1;
            Err(ConstructorError::Clock)
        })
        .await;
        assert!(result.is_err());
        assert_eq!(clocks, usize::from(mode == 1));
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, seq);
        assert_eq!(event_journals(&p).await.len(), 1);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_empty_logout_inserts_missing_state_and_catches_provider_failure_without_rollback_of_prior()
 {
    use vindictus_server::infrastructure::user_captcha as db;
    let (a, p, s) = isolated_game_pool().await;
    let prior = captcha_prior(&p).await;
    captcha_bootstrap(&p).await;
    let loaded = managers::load_captcha(&p, &owner(), prior, &events_profile(), &catalog(), || {
        panic!("unexpected clock")
    })
    .await
    .unwrap();
    sqlx::raw_sql("CREATE FUNCTION reject_captcha_save()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic save failure';END$$;CREATE TRIGGER reject_captcha_save BEFORE INSERT OR UPDATE ON game_user_captcha FOR EACH STATEMENT EXECUTE FUNCTION reject_captcha_save()").execute(&p).await.unwrap();
    assert_eq!(
        db::save_empty_logout(&p, loaded.loaded()).await,
        db::LogoutSave::Caught
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_user_captcha")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 0);
    assert_eq!(event_journals(&p).await.len(), 1);
    sqlx::query("DROP TRIGGER reject_captcha_save ON game_user_captcha")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        db::save_empty_logout(&p, loaded.loaded()).await,
        db::LogoutSave::Saved
    );
    let actual:CaptchaStored=sqlx::query_as("SELECT punished_count,failure_count,last_failure_units300,last_succeed_units300,condition FROM game_user_captcha WHERE uid=31").fetch_one(&p).await.unwrap();
    assert_eq!(actual, (Some(0), Some(0), None, None, Some(0)));
    p.close().await;
    assert_eq!(
        db::save_empty_logout(&p, loaded.loaded()).await,
        db::LogoutSave::Caught
    );
    cleanup_game_pool(a, p, s).await;
}

type CaptchaStored = (
    Option<i16>,
    Option<i16>,
    Option<i64>,
    Option<i64>,
    Option<i16>,
);

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_native_provenance_preserves_reference_and_refuses_drift_without_repair() {
    use vindictus_server::infrastructure::user_captcha as db;
    let (a, p, s) = isolated_game_pool().await;
    let prior = captcha_prior(&p).await;
    let snap=db::read_snapshot(br#"{"format_version":1,"source":"study_heroes.user-captcha","rows":[{"uid":-7,"punished_count":null,"failure_count":255,"last_failure_units300":"-1","last_succeed_units300":null,"condition":255}]}"#.as_slice()).unwrap();
    db::preserve(&p, &snap, false).await.unwrap();
    let state = vindictus_server::domain::initial_captcha::Loaded::construct(
        prior.prior().prior().prior().social(),
        None,
        || panic!("no clock for absent row"),
    )
    .unwrap();
    assert_eq!(
        db::save_empty_logout(&p, &state).await,
        db::LogoutSave::Saved
    );
    let verified = db::preserve(&p, &snap, true).await.unwrap();
    assert_eq!(
        (
            verified.verified,
            verified.native_verified,
            verified.inserted
        ),
        (1, 1, 0)
    );
    assert_eq!(db::preserve(&p, &snap, false).await.unwrap().inserted, 0);
    for change in [
        "UPDATE game_user_captcha SET failure_count=2 WHERE uid=31",
        "UPDATE game_user_captcha_native_state SET saved_sha256=decode(repeat('01',32),'hex') WHERE uid=31",
        "UPDATE game_user_captcha_native_state SET origin_fingerprint=decode(repeat('01',32),'hex') WHERE uid=31",
        "DELETE FROM game_user_captcha WHERE uid=31",
        "DELETE FROM game_user_captcha_native_state WHERE uid=31",
    ] {
        sqlx::query(change).execute(&p).await.unwrap();
        assert!(matches!(
            db::preserve(&p, &snap, true).await,
            Err(db::CaptchaError::Conflict)
        ));
        let mut tx = p.begin().await.unwrap();
        assert!(matches!(
            db::read_initial(&mut tx, &owner()).await,
            Err(db::CaptchaError::Conflict)
        ));
        tx.rollback().await.unwrap();
        assert_eq!(
            db::save_empty_logout(&p, &state).await,
            db::LogoutSave::Saved
        );
    }
    let mut tx = p.begin().await.unwrap();
    assert!(db::read_initial(&mut tx, &owner()).await.unwrap().is_some());
    tx.rollback().await.unwrap();
    // Source fields are still compared exactly, even with an audited native row.
    sqlx::query("UPDATE game_user_captcha SET condition=1 WHERE uid=-7")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        db::preserve(&p, &snap, false).await,
        Err(db::CaptchaError::Conflict)
    ));
    let actual: Option<i16> =
        sqlx::query_scalar("SELECT condition FROM game_user_captcha WHERE uid=-7")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(actual, Some(1), "replay must not repair source drift");
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_native_save_and_receipt_are_atomic_and_require_synthetic_origin() {
    use vindictus_server::infrastructure::user_captcha as db;
    let (a, p, s) = isolated_game_pool().await;
    let prior = captcha_prior(&p).await;
    captcha_bootstrap(&p).await;
    let state = vindictus_server::domain::initial_captcha::Loaded::construct(
        prior.prior().prior().prior().social(),
        None,
        || panic!("no clock"),
    )
    .unwrap();
    let sequences = social_sequences(&p).await;
    sqlx::raw_sql("CREATE FUNCTION reject_native_receipt() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic receipt failure';END$$;CREATE TRIGGER reject_native_receipt BEFORE INSERT ON game_user_captcha_native_state FOR EACH STATEMENT EXECUTE FUNCTION reject_native_receipt()").execute(&p).await.unwrap();
    assert_eq!(
        db::save_empty_logout(&p, &state).await,
        db::LogoutSave::Caught
    );
    let n:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM game_user_captcha)+(SELECT count(*) FROM game_user_captcha_native_state)").fetch_one(&p).await.unwrap();
    assert_eq!(n, 0, "late receipt error rolls back data upsert");
    sqlx::query("DROP TRIGGER reject_native_receipt ON game_user_captcha_native_state")
        .execute(&p)
        .await
        .unwrap();
    for revision in 1..=2i64 {
        assert_eq!(
            db::save_empty_logout(&p, &state).await,
            db::LogoutSave::Saved
        );
        let got: (i64, i64) = sqlx::query_as(
            "SELECT last_cid,revision FROM game_user_captcha_native_state WHERE uid=31",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(got, (1, revision));
    }
    sqlx::query("DELETE FROM game_lab_account_origins WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    assert_eq!(
        db::save_empty_logout(&p, &state).await,
        db::LogoutSave::Caught
    );
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_user_captcha_native_state")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(n, 0);
    assert_eq!(social_sequences(&p).await, sequences);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn captcha_logout_never_overwrites_source_uid_even_with_synthetic_origin() {
    use vindictus_server::infrastructure::user_captcha as db;
    let (a, p, s) = isolated_game_pool().await;
    let prior = captcha_prior(&p).await;
    let snap=db::read_snapshot(br#"{"format_version":1,"source":"study_heroes.user-captcha","rows":[{"uid":31,"punished_count":null,"failure_count":255,"last_failure_units300":null,"last_succeed_units300":null,"condition":255}]}"#.as_slice()).unwrap();
    db::preserve(&p, &snap, false).await.unwrap();
    let state = vindictus_server::domain::initial_captcha::Loaded::construct(
        prior.prior().prior().prior().social(),
        None,
        || panic!("no clock"),
    )
    .unwrap();
    assert_eq!(
        db::save_empty_logout(&p, &state).await,
        db::LogoutSave::Caught
    );
    assert_eq!(
        db::preserve(&p, &snap, true).await.unwrap().native_verified,
        0
    );
    sqlx::query("DELETE FROM game_user_captcha WHERE uid=31")
        .execute(&p)
        .await
        .unwrap();
    let mut tx = p.begin().await.unwrap();
    assert!(matches!(
        db::read_initial(&mut tx, &owner()).await,
        Err(db::CaptchaError::Conflict)
    ));
    tx.rollback().await.unwrap();
    cleanup_game_pool(a, p, s).await;
}

async fn root_sources(p: &PgPool) {
    root_sources_cid(p, 1).await
}
async fn root_sources_cid(p: &PgPool, cid: i64) {
    setup_cid(p, cid).await;
    costume_source(p, false).await;
    empty_vocation_source(p).await;
    use vindictus_server::infrastructure::{
        character_status_effects as status, item_journals as j,
    };
    let snap=status::read_snapshot(br#"{"format_version":1,"source":"study_heroes.status-effects","source_last_value":"100","effects":[]}"#.as_slice()).unwrap();
    status::preserve(p, &snap, false).await.unwrap();
    let snap=j::read_snapshot(br#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":"611635799","character_last":"4419699"}"#.as_slice()).unwrap();
    j::preserve(p, &snap, false).await.unwrap();
    captcha_bootstrap(p).await;
}
async fn transfer_source(p: &PgPool) {
    use vindictus_server::infrastructure::character_transfer_guard as db;
    let v = json!({"format_version":1,"source":"study_heroes.character-transfer-empty","rows":0,"columns":8,"schema_sha256":"a".repeat(64),"procedure_sha256":"6f47c1f5f6926d7d5bd474fa91547c2e31343d3afc0f8853e3a0ffee0370baaa"});
    let snap = db::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).unwrap();
    db::preserve(p, &snap, false).await.unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_transfer_guard_precedes_all_constructor_writes_and_clocks() {
    for initialized in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        root_sources(&p).await;
        if initialized {
            transfer_source(&p).await;
            sqlx::query("INSERT INTO game_character_transfer_info VALUES(31,''::bytea,10,1,''::bytea,20,2,NULL)").execute(&p).await.unwrap();
        }
        let before = counts(&p).await;
        let seq = social_sequences(&p).await;
        let mut obs = vindictus_server::domain::initial_status::Profile::default();
        assert!(matches!(
            managers::load_first_root(
                &p,
                &owner(),
                1,
                &events_profile(),
                &catalog(),
                || panic!("guard must run first"),
                &mut obs
            )
            .await,
            Err(ManagerError::Transfer(_))
        ));
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, seq);
        assert!(event_journals(&p).await.is_empty());
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_and_join_last_cid_preserve_separate_commit_missing_and_existing_prologue() {
    for prior_flag in [None, Some(false), Some(true)] {
        let (a, p, s) = isolated_game_pool().await;
        root_sources(&p).await;
        transfer_source(&p).await;
        if let Some(flag) = prior_flag {
            sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,$1)")
                .bind(flag)
                .execute(&p)
                .await
                .unwrap();
        }
        // Missing UserInfo enters existing-attendance loading when enabled,which
        // remains unsupported. Exercise UpdateLastCID's missing-row source branch
        // under a synthetic disabled Attendance profile instead of bypassing it.
        let prof = if prior_flag.is_none() {
            attendance_profile("7", false)
        } else {
            events_profile()
        };
        let mut obs = vindictus_server::domain::initial_status::Profile::default();
        let root = managers::load_first_root(
            &p,
            &owner(),
            1,
            &prof,
            &catalog(),
            vindictus_server::infrastructure::lab_clock::utc_now,
            &mut obs,
        )
        .await
        .unwrap();
        assert_eq!(
            (root.uid(), root.cid(), root.preset_used_count()),
            (31, 1, 0)
        );
        assert!(root.is_first_login());
        assert!(!root.is_return());
        assert_eq!(root.cafe_type(), 0);
        assert!(!root.is_in_game());
        assert_eq!(root.remove_town_effect_schedule(), 0);
        assert!(!root.needs_user_care_meeting());
        assert_eq!(
            root.managers().prior().loaded().marble().journal_id(),
            4419700
        );
        let joined = managers::commit_join_last_cid(&p, &owner(), root, &prof, &catalog())
            .await
            .unwrap();
        assert_eq!(
            joined.observables().categories(),
            &["Stat", "Skill", "Title", "MicroPlay", "Fishing"]
        );
        assert_eq!(joined.observables().cid(), 1);
        assert_eq!(joined.observables().proxy_count(), 0);
        // A later independent owner-context rollback must not restore LastCID.
        let mut later = p.begin().await.unwrap();
        sqlx::query("UPDATE game_user_info SET last_login_cid=NULL WHERE nexon_sn=31")
            .execute(&mut *later)
            .await
            .unwrap();
        later.rollback().await.unwrap();
        drop(joined);
        let actual: (Option<i64>, bool) = sqlx::query_as(
            "SELECT last_login_cid,prologue_played FROM game_user_info WHERE nexon_sn=31",
        )
        .fetch_one(&p)
        .await
        .unwrap();
        assert_eq!(actual, (Some(1), prior_flag.unwrap_or(false)));
        let n: i64 = sqlx::query_scalar("SELECT count(*) FROM game_micro_play_status")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, 0, "next Join phase not implicitly run");
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_late_captcha_failure_keeps_mission_stats_sp_and_marble_without_last_cid() {
    let (a, p, s) = isolated_game_pool().await;
    root_sources(&p).await;
    transfer_source(&p).await;
    sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("DELETE FROM game_user_captcha_preservation")
        .execute(&p)
        .await
        .unwrap();
    let mut obs = vindictus_server::domain::initial_status::Profile::default();
    assert!(matches!(
        managers::load_first_root(
            &p,
            &owner(),
            1,
            &events_profile(),
            &catalog(),
            vindictus_server::infrastructure::lab_clock::utc_now,
            &mut obs
        )
        .await,
        Err(ManagerError::Captcha(_))
    ));
    let actual = counts(&p).await;
    assert_eq!(actual.0, 1);
    assert_eq!(actual.1, 1);
    assert_eq!(event_journals(&p).await.len(), 1);
    let n: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_user_info WHERE nexon_sn=31 AND last_login_cid IS NOT NULL",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(n, 0);
    cleanup_game_pool(a, p, s).await;
}

async fn micro_join_prior(p: &PgPool) -> managers::LastCidCommitted {
    root_sources(p).await;
    transfer_source(p).await;
    sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
        .execute(p)
        .await
        .unwrap();
    let mut obs = vindictus_server::domain::initial_status::Profile::default();
    let root = managers::load_first_root(
        p,
        &owner(),
        1,
        &events_profile(),
        &catalog(),
        vindictus_server::infrastructure::lab_clock::utc_now,
        &mut obs,
    )
    .await
    .unwrap();
    managers::commit_join_last_cid(p, &owner(), root, &events_profile(), &catalog())
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_micro_separate_clocks_strict_reset_and_raw_ticks_survive_quantization() {
    use vindictus_server::domain::legacy_user_data::SqlDateTime2;
    let boundary = 639017856000000000i64 + 36_000_000_000;
    for (delta, now_delta) in [(-1, 1), (0, 1), (-1, 0)] {
        let (a, p, s) = isolated_game_pool().await;
        let prior = micro_join_prior(&p).await;
        let seq = social_sequences(&p).await;
        let times = [
            boundary + delta,
            boundary + 17,
            boundary + now_delta,
            boundary + 12345,
        ];
        let mut clocks = 0;
        let out = managers::initialize_join_micro(
            &p,
            &owner(),
            prior,
            &events_profile(),
            &catalog(),
            || {
                let v = SqlDateTime2::from_ticks(times[clocks]).unwrap();
                clocks += 1;
                Ok(v)
            },
        )
        .await
        .unwrap();
        let reset = delta < 0 && now_delta > 0;
        assert_eq!(clocks, if reset { 4 } else { 3 });
        assert_eq!(out.started().ticks(), times[if reset { 3 } else { 0 }]);
        assert_eq!(out.reported().ticks(), times[1]);
        assert_eq!(
            out.window().last().ticks(),
            boundary + if now_delta > 0 { 0 } else { -864000000000 }
        );
        let actual:(i64,i64,Option<i32>)=sqlx::query_as("SELECT last_micro_play_started_units300,last_reported_time_units300,report_received_count FROM game_micro_play_status WHERE cid=1").fetch_one(&p).await.unwrap();
        assert_eq!(
            actual,
            (
                out.started().sql_parameter().unwrap().units300(),
                out.reported().sql_parameter().unwrap().units300(),
                None
            )
        );
        assert_eq!(social_sequences(&p).await, seq);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_micro_later_clock_or_submit_failure_keeps_initial_micro_and_last_cid() {
    use vindictus_server::{
        domain::legacy_user_data::SqlDateTime2, infrastructure::lab_clock::ClockError,
    };
    let boundary = 639017856000000000i64 + 36_000_000_000;
    for mode in 0..3 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = micro_join_prior(&p).await;
        if mode == 2 {
            sqlx::raw_sql("CREATE FUNCTION reject_micro_reset() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic reset submit failure';END$$;CREATE TRIGGER reject_micro_reset BEFORE UPDATE ON game_micro_play_status FOR EACH STATEMENT EXECUTE FUNCTION reject_micro_reset()").execute(&p).await.unwrap();
        }
        let mut clocks = 0;
        let result = managers::initialize_join_micro(
            &p,
            &owner(),
            prior,
            &events_profile(),
            &catalog(),
            || {
                clocks += 1;
                if (mode == 0 && clocks == 3) || (mode == 1 && clocks == 4) {
                    return Err(ClockError::Range);
                };
                SqlDateTime2::from_ticks(if clocks <= 2 {
                    boundary - 1
                } else {
                    boundary + 1
                })
                .map_err(|_| ClockError::Range)
            },
        )
        .await;
        assert!(result.is_err());
        assert_eq!(clocks, if mode == 0 { 3 } else { 4 });
        let actual:(i64,i64)=sqlx::query_as("SELECT last_micro_play_started_units300,last_reported_time_units300 FROM game_micro_play_status WHERE cid=1").fetch_one(&p).await.unwrap();
        let seed = SqlDateTime2::from_ticks(boundary - 1)
            .unwrap()
            .sql_parameter()
            .unwrap()
            .units300();
        assert_eq!(actual, (seed, seed));
        let cid: Option<i64> =
            sqlx::query_scalar("SELECT last_login_cid FROM game_user_info WHERE nexon_sn=31")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(cid, Some(1));
        assert_eq!(event_journals(&p).await.len(), 1);
        cleanup_game_pool(a, p, s).await;
    }
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn first_root_missing_transfer_projection_is_not_a_disabled_feature() {
    for missing in ["CharacterTransferSystem", "CharacterExceptSystem"] {
        let (a, p, s) = isolated_game_pool().await;
        root_sources(&p).await;
        transfer_source(&p).await;
        let mut raw = profile_fixture::synthetic();
        raw["state_sha256"] = json!(format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
        ));
        raw["features"]
            .as_array_mut()
            .unwrap()
            .retain(|f| f["key"] != missing);
        raw["enabled_keys"]
            .as_array_mut()
            .unwrap()
            .retain(|k| k != missing);
        let prof = profile_fixture::read(&raw).unwrap();
        let before = counts(&p).await;
        let seq = social_sequences(&p).await;
        let mut obs = vindictus_server::domain::initial_status::Profile::default();
        assert!(matches!(
            managers::load_first_root(
                &p,
                &owner(),
                1,
                &prof,
                &catalog(),
                || panic!("missing projection must fail before clocks"),
                &mut obs
            )
            .await,
            Err(ManagerError::Conflict)
        ));
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, seq);
        assert!(event_journals(&p).await.is_empty());
        cleanup_game_pool(a, p, s).await;
    }
}

async fn newbie_reference(p: &PgPool, uid: i32) {
    use vindictus_server::infrastructure::newbie_attendance as db;
    let raw = json!({"format_version":1,"source":"study_heroes.newbie-attendance","rows":[{"uid":uid,"event_version":7,"start_ticks":"0","end_ticks":"200"}]});
    let snap = db::read_snapshot(serde_json::to_vec(&raw).unwrap().as_slice()).unwrap();
    db::preserve(p, &snap, false).await.unwrap();
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn attendance_existing_empty_history_supports_missing_info_and_previous_last_cid_without_clock()
 {
    use vindictus_server::domain::initial_attendance::Outcome;
    for old_cid in [None, Some(77i64)] {
        let (a, p, s) = isolated_game_pool().await;
        let prior = attendance_prior(&p).await;
        newbie_reference(&p, -7).await;
        if let Some(cid) = old_cid {
            sqlx::query("INSERT INTO game_user_info VALUES(31,$1,true)")
                .bind(cid)
                .execute(&p)
                .await
                .unwrap();
        }
        let before = counts(&p).await;
        let seq = social_sequences(&p).await;
        let out =
            managers::load_attendance(&p, &owner(), prior, &events_profile(), &catalog(), || {
                panic!("existing get has no clock")
            })
            .await
            .unwrap();
        assert_eq!(out.loaded().outcome(), Outcome::ExistingAbsent);
        assert_eq!(out.loaded().instance_count(), 1);
        assert!(out.loaded().matches_social(out.prior().prior().social()));
        assert_eq!(counts(&p).await, before);
        assert_eq!(social_sequences(&p).await, seq);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn attendance_existing_provider_failure_is_caught_but_present_reference_remains_unsupported()
{
    use vindictus_server::domain::initial_attendance::Outcome;
    for fail in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        let prior = attendance_prior(&p).await;
        newbie_reference(&p, 31).await;
        sqlx::query("INSERT INTO game_user_info VALUES(31,77,true)")
            .execute(&p)
            .await
            .unwrap();
        if fail {
            sqlx::raw_sql("ALTER TABLE game_newbie_attendance_info RENAME TO game_newbie_attendance_saved;CREATE FUNCTION fail_existing_lookup(INTEGER) RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE AS $$BEGIN RAISE EXCEPTION 'synthetic provider failure';END$$;CREATE VIEW game_newbie_attendance_info AS SELECT * FROM game_newbie_attendance_saved WHERE fail_existing_lookup(uid)").execute(&p).await.unwrap();
        }
        let before = counts(&p).await;
        let result =
            managers::load_attendance(&p, &owner(), prior, &events_profile(), &catalog(), || {
                panic!("no existing clock")
            })
            .await;
        if fail {
            assert_eq!(
                result.unwrap().loaded().outcome(),
                Outcome::CaughtExistingLookupError
            );
        } else {
            assert!(matches!(
                result,
                Err(ManagerError::NewbieAttendance(
                    vindictus_server::infrastructure::newbie_attendance::AttendanceError::Present
                ))
            ));
        }
        assert_eq!(counts(&p).await, before);
        cleanup_game_pool(a, p, s).await;
    }
}

fn title_goals_profile(
    prof: &Profile,
    ids: &[(i32, Vec<i32>)],
) -> vindictus_server::infrastructure::fiona_title_preset::GoalProfile {
    use vindictus_server::infrastructure::fiona_title_preset::GoalProfile;
    let code: Value = serde_json::from_str(include_str!(
        "../../src/domain/fiona_initializer_sources.json"
    ))
    .unwrap();
    let hash = prof
        .fingerprint()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let data = json!({"format_version":1,"source":"heroesContents.initial-title-goals","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"initializer_sha256":hash,"title_manager_sha256":code["titles"]["sha256"],"contents_sha256":code["contents"]["sha256"],"titles":ids.iter().map(|(id,goals)|json!({"title_id":id,"goal_ids":goals})).collect::<Vec<_>>()});
    GoalProfile::read(
        serde_json::to_vec(&data).unwrap().as_slice(),
        prof,
        &catalog(),
    )
    .unwrap()
}
async fn title_join_prior(p: &PgPool) -> managers::MicroJoined {
    let prior = micro_join_prior(p).await;
    managers::initialize_join_micro(
        p,
        &owner(),
        prior,
        &events_profile(),
        &catalog(),
        vindictus_server::infrastructure::lab_clock::utc_now,
    )
    .await
    .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn title_preset_phase_builds_shared_goals_without_sql_progress_and_calls_detached_observable()
{
    use vindictus_server::infrastructure::fiona_title_preset as titles;
    let (a, p, s) = isolated_game_pool().await;
    let prior = title_join_prior(&p).await;
    let prof = events_profile();
    let goals = title_goals_profile(&prof, &[(17, vec![34, 35])]);
    let seq = social_sequences(&p).await;
    let out = titles::give_presets(&p, &owner(), prior, &prof, &catalog(), &goals)
        .await
        .unwrap();
    assert!(!out.source_caught());
    assert!(out.title_updated());
    assert_eq!((out.logs_written(), out.logs_failed()), (1, 0));
    assert_eq!(out.cache().titles().count(), 1);
    assert_eq!(out.cache().goals().count(), 2);
    let title = out.cache().titles().next().unwrap();
    assert_eq!((title.cid(), title.id(), title.acquired()), (1, 17, false));
    for g in title.goals() {
        assert_eq!((g.cid(), g.progress()), (1, 0));
        assert!(
            out.cache()
                .goals()
                .any(|other| std::sync::Arc::ptr_eq(g, other))
        );
    }
    let actual:(i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM game_titles WHERE cid=1),(SELECT count(*) FROM game_title_goal_progress WHERE cid=1)").fetch_one(&p).await.unwrap();
    assert_eq!(actual, (1, 0));
    assert_eq!(social_sequences(&p).await, seq);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn title_preset_provider_failures_keep_insert_commit_and_catch_without_false_observable() {
    use vindictus_server::infrastructure::fiona_title_preset as titles;
    for mode in 0..2 {
        let (a, p, s) = isolated_game_pool().await;
        let prior = title_join_prior(&p).await;
        let prof = events_profile();
        let goals = title_goals_profile(&prof, &[(17, vec![34])]);
        if mode == 0 {
            sqlx::raw_sql("CREATE FUNCTION fail_title_insert()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic insert failure';END$$;CREATE TRIGGER fail_title_insert BEFORE INSERT ON game_titles FOR EACH STATEMENT EXECUTE FUNCTION fail_title_insert()").execute(&p).await.unwrap();
        } else {
            // Empty preflight succeeds;only enumeration of the committed new row fails.
            sqlx::raw_sql("ALTER TABLE game_titles RENAME TO game_titles_saved;CREATE FUNCTION fail_title_result(INTEGER,BOOLEAN)RETURNS BOOLEAN LANGUAGE plpgsql VOLATILE AS $$BEGIN RAISE EXCEPTION 'synthetic result failure';END$$;CREATE VIEW game_titles AS SELECT cid,title_id,fail_title_result(title_id,acquired) AS acquired,acquired_time_units300,acquired_quest_utf16,expire_ticks FROM game_titles_saved;CREATE FUNCTION insert_title_view()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN INSERT INTO game_titles_saved VALUES(NEW.cid,NEW.title_id,NEW.acquired,NEW.acquired_time_units300,NEW.acquired_quest_utf16,NEW.expire_ticks);RETURN NEW;END$$;CREATE TRIGGER insert_title_view INSTEAD OF INSERT ON game_titles FOR EACH ROW EXECUTE FUNCTION insert_title_view()").execute(&p).await.unwrap();
        }
        let out = titles::give_presets(&p, &owner(), prior, &prof, &catalog(), &goals)
            .await
            .unwrap();
        assert!(out.source_caught());
        assert!(!out.title_updated());
        assert_eq!(out.cache().titles().count(), 0);
        assert_eq!(out.logs_written(), 0);
        let table = if mode == 0 {
            "game_titles"
        } else {
            "game_titles_saved"
        };
        let n: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table} WHERE cid=1"))
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(n, if mode == 0 { 0 } else { 1 });
        let cid: Option<i64> =
            sqlx::query_scalar("SELECT last_login_cid FROM game_user_info WHERE nexon_sn=31")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(cid, Some(1));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn title_preset_log_failure_continues_cache_and_observable_without_replaying_journals() {
    use vindictus_server::infrastructure::fiona_title_preset as titles;
    let (a, p, s) = isolated_game_pool().await;
    let prior = title_join_prior(&p).await;
    let prof = events_profile();
    let goals = title_goals_profile(&prof, &[(17, vec![])]);
    let journal = event_journals(&p).await;
    sqlx::query("CREATE FUNCTION reject_preset_log()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic log failure';END$$").execute(&p).await.unwrap();
    for table in ["game_character_title_log01", "game_character_title_log02"] {
        sqlx::query(&format!("CREATE TRIGGER reject_preset_log BEFORE INSERT ON {table} FOR EACH STATEMENT EXECUTE FUNCTION reject_preset_log()")).execute(&p).await.unwrap();
    }
    let out = titles::give_presets(&p, &owner(), prior, &prof, &catalog(), &goals)
        .await
        .unwrap();
    assert!(!out.source_caught());
    assert!(out.title_updated());
    assert_eq!((out.logs_written(), out.logs_failed()), (0, 1));
    assert_eq!(out.cache().titles().count(), 1);
    assert_eq!(event_journals(&p).await, journal);
    cleanup_game_pool(a, p, s).await;
}

mod prefix_loader {
    include!("../fixtures/item_loader_support.rs");
}
fn prefix_content() -> (
    vindictus_server::infrastructure::fiona_appearance::Profile,
    Package,
) {
    super::fiona_inventory_delivery::initial_appearance_inputs()
}
const PREFIX_CID: i64 = ((638448480000000000_i64 - 8) | 65535) + 8;
async fn prefix_prior(
    p: &PgPool,
    changes: &[(&str, bool)],
) -> (
    vindictus_server::infrastructure::fiona_title_preset::TitlesJoined,
    Profile,
) {
    use vindictus_server::infrastructure::{item_loader_state, item_state};
    root_sources_cid(p, PREFIX_CID).await;
    transfer_source(p).await;
    let mut raw = profile_fixture::synthetic();
    raw["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&state_export()).unwrap())
    ));
    for (k, value) in changes {
        raw["features"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|f| f["key"] == *k)
            .unwrap()["value"] = if *value {
            json!("synthetic-on")
        } else {
            json!("synthetic-off")
        }
    }
    let prof = profile_fixture::read(&raw).unwrap();
    sqlx::query("INSERT INTO game_user_info VALUES(31,NULL,true)")
        .execute(p)
        .await
        .unwrap();
    let mut obs = vindictus_server::domain::initial_status::Profile::default();
    let root = managers::load_first_root(
        p,
        &owner(),
        PREFIX_CID,
        &prof,
        &catalog(),
        vindictus_server::infrastructure::lab_clock::utc_now,
        &mut obs,
    )
    .await
    .unwrap();
    let last = managers::commit_join_last_cid(p, &owner(), root, &prof, &catalog())
        .await
        .unwrap();
    let micro = managers::initialize_join_micro(
        p,
        &owner(),
        last,
        &prof,
        &catalog(),
        vindictus_server::infrastructure::lab_clock::utc_now,
    )
    .await
    .unwrap();
    let goals = title_goals_profile(&prof, &[(17, vec![34])]);
    let titles = vindictus_server::infrastructure::fiona_title_preset::give_presets(
        p,
        &owner(),
        micro,
        &prof,
        &catalog(),
        &goals,
    )
    .await
    .unwrap();
    let v = json!({"format_version":1,"source":"study_heroes.item-state","attribute_last_value":null,"storage_last_value":null,"items":[],"attributes":[],"equipment":[],"storage":[]});
    let snap = item_state::read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).unwrap();
    item_state::preserve(p, &snap, false).await.unwrap();
    let snap = item_loader_state::read_snapshot(
        serde_json::to_vec(&prefix_loader::empty())
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    item_loader_state::preserve(p, &snap, false).await.unwrap();
    (titles, prof)
}
async fn prefix_sequences(p: &PgPool) -> ((i64, bool), (i64, bool)) {
    (
        sqlx::query_as("SELECT last_value,is_called FROM game_item_journal_id")
            .fetch_one(p)
            .await
            .unwrap(),
        sqlx::query_as("SELECT last_value,is_called FROM game_character_journal_id")
            .fetch_one(p)
            .await
            .unwrap(),
    )
}
fn prefix_utc() -> Result<vindictus_server::domain::legacy_user_data::SqlDateTime2, ()> {
    vindictus_server::domain::legacy_user_data::SqlDateTime2::from_ticks(638448480000000001)
        .map_err(|_| ())
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_composes_shared_graph_stores_save_queue_and_manufacture_in_source_order() {
    use vindictus_server::infrastructure::fiona_item_bootstrap as prefix;
    for shared in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        let (titles, prof) = prefix_prior(
            &p,
            &[
                ("SharedInventory", shared),
                ("ProcessAfterDeleteItem", false),
            ],
        )
        .await;
        let (app, cat) = prefix_content();
        let before = prefix_sequences(&p).await;
        let mut calls = 0;
        let mut out = prefix::load_prefix(&p, &owner(), titles, &prof, &cat, &app, || {
            calls += 1;
            prefix_utc()
        })
        .await
        .unwrap();
        assert_eq!(calls, 1);
        assert_eq!((out.uid(), out.cid()), (31, PREFIX_CID));
        assert!(out.bindings_valid());
        assert!(!out.item_published());
        assert!(!out.active());
        assert_eq!(out.login_time().ticks(), 638448480000000001);
        assert_eq!(
            out.latest_sort().units300(),
            vindictus_server::domain::legacy_user_data::SqlDateTime::MIN
        );
        assert_eq!(out.gear().gear_slots(), 1);
        assert_eq!(out.storage().bag_slots(), 144);
        assert_eq!(out.storage().other_stores(), [0; 4]);
        assert_eq!(out.manufacture_counts(), [0; 3]);
        assert_eq!(out.shared_initialized(), shared);
        assert_eq!(
            out.shared_cache_counts(),
            if shared {
                Some([0, 0, 0, 0, 0, 0, 1])
            } else {
                None
            }
        );
        assert_eq!(out.deleted_count(), None);
        assert_eq!(out.log_write_calls(), 1);
        assert_eq!(out.transaction_scope().unwrap(), None);
        let categories = out.categories().unwrap();
        assert_eq!(categories.len(), if shared { 9 } else { 8 });
        assert!(
            categories
                .iter()
                .all(|(id, c)| *id == if *c == "SharedInven" { 31 } else { PREFIX_CID })
        );
        assert_eq!(out.pending_sync().unwrap(), 1);
        assert_eq!(out.drain_detached_sync().unwrap(), 1);
        assert_eq!(out.pending_sync().unwrap(), 0);
        assert_eq!(out.sync_drained(), 1);
        let after = prefix_sequences(&p).await;
        assert_eq!(after.0.0, before.0.0 + i64::from(before.0.1));
        assert_eq!(after.1.0, before.1.0 + i64::from(before.1.1));
        let j:i64=sqlx::query_scalar("SELECT count(*) FROM game_item_journal01 WHERE category_utf16=$1 UNION ALL SELECT count(*) FROM game_item_journal02 WHERE category_utf16=$1").bind("ManufactureReport".encode_utf16().flat_map(u16::to_le_bytes).collect::<Vec<_>>()).fetch_all(&p).await.unwrap().iter().sum();
        assert_eq!(j, 1);
        let n:i64=sqlx::query_scalar("SELECT (SELECT count(*) FROM game_items)+(SELECT count(*) FROM game_item_log01)+(SELECT count(*) FROM game_item_log02)").fetch_one(&p).await.unwrap();
        assert_eq!(n, 0);
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_stale_owner_and_clock_failure_stop_before_journal_without_repair() {
    use vindictus_server::infrastructure::fiona_item_bootstrap::{self as prefix, BootstrapError};
    for stale in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        let (titles, prof) = prefix_prior(&p, &[]).await;
        let (app, cat) = prefix_content();
        let before = prefix_sequences(&p).await;
        if stale {
            sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=31")
                .execute(&p)
                .await
                .unwrap();
        }
        let mut calls = 0;
        let result = prefix::load_prefix(&p, &owner(), titles, &prof, &cat, &app, || {
            calls += 1;
            Err::<vindictus_server::domain::legacy_user_data::SqlDateTime2, _>(())
        })
        .await;
        assert!(if stale {
            matches!(
                result,
                Err(BootstrapError::Owner(state::StateError::Ownership))
            )
        } else {
            matches!(result, Err(BootstrapError::Clock))
        });
        assert_eq!(calls, if stale { 0 } else { 1 });
        assert_eq!(prefix_sequences(&p).await, before);
        let cid: Option<i64> =
            sqlx::query_scalar("SELECT last_login_cid FROM game_user_info WHERE nexon_sn=31")
                .fetch_one(&p)
                .await
                .unwrap();
        assert_eq!(cid, Some(PREFIX_CID));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_collection_or_dirty_inventory_failure_retains_only_constructor_identity() {
    use vindictus_server::infrastructure::fiona_item_bootstrap as prefix;
    for dirty in [false, true] {
        let (a, p, s) = isolated_game_pool().await;
        let (titles, prof) = prefix_prior(&p, &[]).await;
        let (app, cat) = prefix_content();
        let before = prefix_sequences(&p).await;
        if dirty {
            sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,tradable)VALUES(77,$1,638448480000016391,1,0,0,true)").bind(vec![65u8,0]).execute(&p).await.unwrap();
        } else {
            sqlx::query("DELETE FROM game_mailboxes WHERE owner_id=638448480000016391")
                .execute(&p)
                .await
                .unwrap();
        }
        assert!(
            prefix::load_prefix(&p, &owner(), titles, &prof, &cat, &app, prefix_utc)
                .await
                .is_err()
        );
        let after = prefix_sequences(&p).await;
        assert_eq!(after.0.0, before.0.0 + i64::from(before.0.1));
        assert_eq!(after.1, before.1);
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_items")
            .fetch_one(&p)
            .await
            .unwrap();
        assert_eq!(count, if dirty { 1 } else { 0 });
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_manufacture_failure_keeps_save_and_independent_report_journal() {
    use vindictus_server::infrastructure::fiona_item_bootstrap::{self as prefix, BootstrapError};
    let (a, p, s) = isolated_game_pool().await;
    let (titles, prof) = prefix_prior(&p, &[]).await;
    let (app, cat) = prefix_content();
    let before = prefix_sequences(&p).await;
    sqlx::raw_sql("ALTER TABLE game_manufacture RENAME TO game_manufacture_saved;CREATE FUNCTION fail_manufacture_cid()RETURNS BIGINT LANGUAGE plpgsql VOLATILE AS $$BEGIN RAISE EXCEPTION 'synthetic provider failure';END$$;CREATE VIEW game_manufacture AS SELECT 0 AS row_id,fail_manufacture_cid()AS cid,''::bytea AS manufacture_utf16,0 AS grade,0 AS experience_point").execute(&p).await.unwrap();
    assert!(matches!(
        prefix::load_prefix(&p, &owner(), titles, &prof, &cat, &app, prefix_utc).await,
        Err(BootstrapError::Database(_))
    ));
    let after = prefix_sequences(&p).await;
    assert_eq!(after.0.0, before.0.0 + i64::from(before.0.1));
    assert_eq!(after.1.0, before.1.0 + i64::from(before.1.1));
    let cid: Option<i64> =
        sqlx::query_scalar("SELECT last_login_cid FROM game_user_info WHERE nexon_sn=31")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(cid, Some(PREFIX_CID));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_failed_journals_use_source_sentinel_and_continue_empty_load() {
    use vindictus_server::infrastructure::fiona_item_bootstrap as prefix;
    let (a, p, s) = isolated_game_pool().await;
    let (titles, prof) = prefix_prior(&p, &[]).await;
    let (app, cat) = prefix_content();
    sqlx::query("DELETE FROM game_journal_identity")
        .execute(&p)
        .await
        .unwrap();
    let before = prefix_sequences(&p).await;
    let out = prefix::load_prefix(&p, &owner(), titles, &prof, &cat, &app, prefix_utc)
        .await
        .unwrap();
    assert_eq!(
        (out.operation_journal(), out.manufacture_journal()),
        (-1, -1)
    );
    assert_eq!(prefix_sequences(&p).await, before);
    assert_eq!(out.pending_sync().unwrap(), 1);
    assert!(out.bindings_valid());
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_prefix_rejects_non_character_entity_before_clock_and_journal() {
    use vindictus_server::infrastructure::{
        fiona_item_bootstrap as prefix, fiona_title_preset as titles,
    };
    let (a, p, s) = isolated_game_pool().await;
    let prior = title_join_prior(&p).await;
    let prof = events_profile();
    let goals = title_goals_profile(&prof, &[(17, vec![])]);
    let joined = titles::give_presets(&p, &owner(), prior, &prof, &catalog(), &goals)
        .await
        .unwrap();
    let (app, cat) = prefix_content();
    let before = prefix_sequences(&p).await;
    let mut calls = 0;
    let result = prefix::load_prefix(&p, &owner(), joined, &prof, &cat, &app, || {
        calls += 1;
        prefix_utc()
    })
    .await;
    assert!(matches!(result, Err(prefix::BootstrapError::Conflict)));
    assert_eq!(calls, 0);
    assert_eq!(prefix_sequences(&p).await, before);
    cleanup_game_pool(a, p, s).await;
}
