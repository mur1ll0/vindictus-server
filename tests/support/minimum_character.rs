use super::{
    cleanup_game_pool, fiona_inventory_delivery as fixture, initial_inventory, isolated_game_pool,
};
use sqlx::PgPool;
use std::sync::Arc;
use vindictus_server::{
    application::{
        character_creation::EntityAuthority,
        creation_lifecycle::{MinimumOutcome, MinimumRequest},
    },
    domain::{item_colors::ColorRandom, legacy_user_data::SqlDateTime2},
    infrastructure::{
        character_lobby::QueryConfig,
        character_skills,
        minimum_assets::{Assets, Parts},
        minimum_character as minimum, minimum_character_list as list,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        character_creation::{self as wire, Kind, Message},
        character_list,
    },
};
pub(super) async fn setup(p: &PgPool) {
    fixture::appearance_setup(p, true).await;
    character_skills::preserve(p,&character_skills::read_snapshot(br#"{"format_version":1,"source":"study_heroes.skill-state","sp_source_last_value":null,"skills":[],"sp_slots":[]}"#.as_slice()).unwrap(),false).await.unwrap();
    super::fiona_managers::empty_vocation_source(p).await;
}
fn config() -> QueryConfig {
    QueryConfig {
        max_free: 5,
        care_clock: None,
    }
}
fn now() -> SqlDateTime2 {
    SqlDateTime2::from_ticks(638448480000000000).unwrap()
}
pub(super) fn assets() -> Arc<Assets> {
    let i = fixture::AppearanceInputs::new();
    Arc::new(
        Assets::from_parts(Parts {
            initializer: fixture::profile(false),
            catalog: i.catalog,
            stats: i.stats,
            colors: i.colors,
            inventory: i.inventory,
            appearance: i.profile,
            entity_service_id: 77,
            arithmetic: None,
            color_seed: 17,
        })
        .unwrap(),
    )
}
pub(super) fn body() -> zeroize::Zeroizing<Vec<u8>> {
    let i = fixture::AppearanceInputs::new();
    wire::encode(
        Kind::Create,
        Some(&Message::Create {
            name: Some("Murael"),
            template: Some(i.template),
            preset: false,
            premium: false,
        }),
        DEFAULT_MAX_BODY,
    )
    .unwrap()
}
async fn initialize(p: &PgPool) -> Result<minimum::Initialized, minimum::MinimumError> {
    let i = fixture::AppearanceInputs::new();
    minimum::initialize(
        p,
        &initial_inventory::key(),
        1,
        minimum::Inputs {
            initializer: &fixture::profile(false),
            appearance: i.request(),
            stats: &i.stats,
        },
        &mut EntityAuthority::new(77),
        &mut ColorRandom::from_seed(17),
        minimum::Clock {
            utc: &mut || Ok(now()),
            civil: &mut || Ok(now()),
        },
    )
    .await
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_create_persists_inventory_appearance_skills_and_filled_list() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let result = initialize(&p).await.unwrap();
    assert_eq!(result.cid(), 1);
    assert!(result.items() > 0);
    assert_eq!(result.skills(), 1);
    let bytes = list::query(
        &p,
        &initial_inventory::key(),
        &fixture::profile(false),
        config(),
        1440,
        || Ok(now()),
    )
    .await
    .unwrap();
    let decoded = character_list::decode(&bytes, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap();
    let summary = decoded.characters.unwrap().pop().unwrap().unwrap();
    assert_eq!(
        (
            summary.cid,
            summary.character_id,
            summary.base_character,
            summary.level,
            summary.level_up_exp
        ),
        (1, Some("Murael"), 1, 1, 81)
    );
    assert!(summary.costume.is_some());
    assert_eq!(
        (summary.vocation_class, summary.vocation_level_up_exp),
        (-1, -1)
    );
    assert!(initialize(&p).await.is_err());
    let persisted:(bool,i64,i64)=sqlx::query_as("SELECT initialized,(SELECT count(*)FROM game_items),(SELECT count(*)FROM game_skills)FROM game_minimum_character_initialization WHERE cid=1").fetch_one(&p).await.unwrap();
    assert_eq!(persisted, (true, result.items() as i64, 1));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_list_refuses_partial_unmarked_and_stale_owner() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    assert!(
        list::query(
            &p,
            &initial_inventory::key(),
            &fixture::profile(false),
            config(),
            0,
            || Ok(now())
        )
        .await
        .is_err()
    );
    initialize(&p).await.unwrap();
    sqlx::query("UPDATE game_minimum_character_initialization SET initialized=false")
        .execute(&p)
        .await
        .unwrap();
    assert!(
        list::query(
            &p,
            &initial_inventory::key(),
            &fixture::profile(false),
            config(),
            0,
            || Ok(now())
        )
        .await
        .is_err()
    );
    sqlx::raw_sql("UPDATE game_minimum_character_initialization SET initialized=true;UPDATE game_users SET service_id=service_id").execute(&p).await.unwrap();
    assert!(
        list::query(
            &p,
            &initial_inventory::key(),
            &fixture::profile(false),
            config(),
            0,
            || Ok(now())
        )
        .await
        .is_err()
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_late_failure_keeps_marker_partial_and_requires_compensation() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION reject_minimum_skill() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic failure';END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER reject_minimum_skill BEFORE INSERT ON game_skills FOR EACH ROW EXECUTE FUNCTION reject_minimum_skill()").execute(&p).await.unwrap();
    assert!(initialize(&p).await.is_err());
    let marker: bool = sqlx::query_scalar(
        "SELECT initialized FROM game_minimum_character_initialization WHERE cid=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert!(!marker);
    assert!(
        list::query(
            &p,
            &initial_inventory::key(),
            &fixture::profile(false),
            config(),
            0,
            || Ok(now())
        )
        .await
        .is_err()
    );
    assert!(initialize(&p).await.is_err());
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_lifecycle_create_returns_list_and_compensates_late_failure_before_retry() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::raw_sql("DELETE FROM game_characters;DELETE FROM game_my_settings")
        .execute(&p)
        .await
        .unwrap();
    let (session, revision, _) = super::creation_lifecycle::setup(&p).await;
    let runtime = super::creation_lifecycle::runtime(&p, &super::creation_lifecycle::package());
    sqlx::query("CREATE FUNCTION fail_minimum_costume() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN RAISE EXCEPTION 'synthetic failure';END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER fail_minimum_costume BEFORE INSERT ON game_costumes FOR EACH ROW EXECUTE FUNCTION fail_minimum_costume()").execute(&p).await.unwrap();
    let request = || MinimumRequest {
        revision,
        name: "Murael",
        assets: assets(),
        body: body(),
        offset: 0,
        config: config(),
        delete_delay: 1440,
    };
    assert!(matches!(
        runtime
            .run_minimum_admitted(&session, request())
            .await
            .unwrap(),
        MinimumOutcome::InitializationFailed(_)
    ));
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT(SELECT count(*)FROM game_characters),(SELECT count(*)FROM game_minimum_character_initialization),(SELECT count(*)FROM game_items)").fetch_one(&p).await.unwrap();
    assert_eq!(counts, (0, 0, 0));
    sqlx::query("DROP TRIGGER fail_minimum_costume ON game_costumes")
        .execute(&p)
        .await
        .unwrap();
    let MinimumOutcome::Created(created) = runtime
        .run_minimum_admitted(&session, request())
        .await
        .unwrap()
    else {
        panic!("minimum create failed")
    };
    let decoded = character_list::decode(&created.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap();
    assert_eq!(
        decoded.characters.unwrap()[0].as_ref().unwrap().cid,
        created.cid
    );
    runtime.shutdown().await.unwrap();
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_cancel_joins_initializer_before_deleting_committed_base() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::raw_sql("DELETE FROM game_characters;DELETE FROM game_my_settings")
        .execute(&p)
        .await
        .unwrap();
    let (session, revision, _) = super::creation_lifecycle::setup(&p).await;
    let runtime = Arc::new(super::creation_lifecycle::runtime(
        &p,
        &super::creation_lifecycle::package(),
    ));
    let mut blocker = p.begin().await.unwrap();
    sqlx::query("LOCK TABLE game_costumes IN SHARE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let r = Arc::clone(&runtime);
    let job = tokio::spawn(async move {
        r.run_minimum_admitted(
            &session,
            MinimumRequest {
                revision,
                name: "Murael",
                assets: assets(),
                body: body(),
                offset: 0,
                config: config(),
                delete_delay: 1440,
            },
        )
        .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(8), async {
        loop {
            let n: i64 =
                sqlx::query_scalar("SELECT count(*)FROM game_minimum_character_initialization")
                    .fetch_one(&p)
                    .await
                    .unwrap();
            if n == 1 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    job.abort();
    let _ = job.await;
    blocker.rollback().await.unwrap();
    runtime.drain().await.unwrap();
    runtime.shutdown().await.unwrap();
    let n:(i64,i64,i64)=sqlx::query_as("SELECT(SELECT count(*)FROM game_characters),(SELECT count(*)FROM game_items),(SELECT count(*)FROM game_minimum_character_initialization)").fetch_one(&p).await.unwrap();
    assert_eq!(n, (0, 0, 0));
    cleanup_game_pool(a, p, s).await;
}
