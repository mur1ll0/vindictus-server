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
                assert!(matches!(result, Err(ManagerError::Attendance(_))));
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
