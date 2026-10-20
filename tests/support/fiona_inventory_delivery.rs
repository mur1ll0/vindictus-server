type StoredLog = (i64, i16, i16, Option<i32>, Option<i32>);
type StoredLedger = (
    i64,
    i64,
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);
use super::{cleanup_game_pool, initial_inventory as fixture, isolated_game_pool};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use vindictus_server::{
    application::{
        character_creation::EntityAuthority,
        inventory_delivery::{self as prep, DeliveryClock, Inputs, Journals, Prepared},
    },
    domain::{
        initial_inventory::{InitialRequest, InventoryError},
        item_colors::{ColorRandom, Materials},
        item_stats::ItemStat,
        legacy_user_data::SqlDateTime2,
    },
    infrastructure::{
        fiona_character_state as state,
        fiona_initializer_profile::Profile,
        fiona_inventory_delivery::{self as delivery, DeliveryError},
        item_journals,
    },
};
mod profile_fixture {
    include!("../fixtures/initializer_profile_support.rs");
}
fn snapshot() -> Value {
    json!({"format_version":1,"source":"study_heroes.character-initialization","titles":[],"goals":[],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"micro":[],"missions":[],"assignments":[]})
}
pub(super) fn profile(shared: bool) -> Profile {
    profile_with_base(shared, [1, 2, 3, 4, 5, 6])
}
pub(super) fn profile_with_base(shared: bool, base: [i32; 6]) -> Profile {
    let mut v = profile_fixture::synthetic();
    v["base_stats"] = json!(base);
    v["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"DeleteCharacterDelay","value":"123"}));
    v["state_sha256"] = json!(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&snapshot()).unwrap())
    ));
    if shared {
        for f in v["features"].as_array_mut().unwrap() {
            if f["key"] == "SharedInventory" {
                f["value"] = json!("synthetic-on");
            }
        }
    }
    profile_fixture::read(&v).unwrap()
}
async fn setup(p: &PgPool) {
    fixture::setup(p, true).await;
    state::preserve(
        p,
        &state::read_snapshot(serde_json::to_vec(&snapshot()).unwrap().as_slice()).unwrap(),
        false,
    )
    .await
    .unwrap();
    item_journals::preserve(p,&item_journals::read_snapshot(br#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":null,"character_last":null}"#.as_slice()).unwrap(),false).await.unwrap();
}
fn prepared(j: Journals) -> Result<Prepared, DeliveryError> {
    let keys = ["synthetic_eq", "synthetic_stack", "synthetic_cash"].map(|s| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    });
    let stat = ItemStat::from_raw(&[], &[], [50, 0, 0, 0]).unwrap();
    let requests = keys
        .iter()
        .enumerate()
        .map(|(i, k)| InitialRequest {
            class: k,
            count: [1, 5, 2][i],
            max_stack: [1, 3, 99][i],
            unique: 0,
            storage: if i == 2 { 2 } else { 1 },
            expire_in: None,
            equipment: i == 0,
            base: &stat,
            materials: [None; 3],
            gold: false,
        })
        .collect::<Vec<_>>();
    Ok(prep::prepare(
        Inputs {
            requests: &requests,
            materials: &Materials::default(),
            maximum_durability: 100,
            bundle_exceed: 0,
        },
        1,
        j,
        &mut ColorRandom::from_seed(1),
        &mut DeliveryClock {
            authority: &mut EntityAuthority::new(17),
            utc: &mut || Ok(SqlDateTime2::from_ticks(638448480000000000).unwrap()),
            civil: &mut || Ok(SqlDateTime2::from_ticks(638000000000017001).unwrap()),
        },
    )?)
}
async fn count(p: &PgPool, f: &str) -> i64 {
    let q = match f {
        "items" => "SELECT count(*) FROM game_items",
        "logs" => {
            "SELECT (SELECT count(*)FROM game_item_log01)+(SELECT count(*)FROM game_item_log02)"
        }
        "ledgers" => {
            "SELECT (SELECT count(*)FROM game_item_ledger01)+(SELECT count(*)FROM game_item_ledger02)"
        }
        _ => {
            "SELECT (SELECT count(*)FROM game_item_journal01)+(SELECT count(*)FROM game_item_journal02)"
        }
    };
    sqlx::query_scalar(q).fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_commits_before_logs_and_captures_original_requests_and_last_counts() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    sqlx::query("CREATE FUNCTION delivery_saved_first()RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF (SELECT count(*) FROM game_items)<>4 THEN RAISE EXCEPTION 'inventory not committed';END IF;RETURN NEW;END$$").execute(&p).await.unwrap();
    for t in [
        "game_item_log01",
        "game_item_log02",
        "game_item_ledger01",
        "game_item_ledger02",
    ] {
        sqlx::query(&format!("CREATE TRIGGER delivery_saved_first BEFORE INSERT ON {t} FOR EACH ROW EXECUTE FUNCTION delivery_saved_first()")).execute(&p).await.unwrap();
    }
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared)
        .await
        .unwrap();
    assert_eq!(o.prepared.inventory().items().len(), 4);
    assert_eq!(
        (o.item_logs.written, o.item_logs.failed, o.ledgers.written),
        (4, 0, 3)
    );
    assert_eq!(count(&p, "items").await, 4);
    let rows:Vec<StoredLog>=sqlx::query_as("SELECT journal_id,event,detail,after_inventory,after_shared FROM game_item_log01 UNION ALL SELECT journal_id,event,detail,after_inventory,after_shared FROM game_item_log02").fetch_all(&p).await.unwrap();
    assert_eq!(rows.iter().filter(|r| r.3 == Some(5)).count(), 1);
    assert!(
        rows.iter()
            .all(|r| r.0 == 611635750 && r.1 == 19 && r.2 == 1 && r.4.is_none())
    );
    let rows:Vec<StoredLedger>=sqlx::query_as("SELECT journal_id,item_id,count,color1,color2,color3,bundle_item_count FROM game_item_ledger01 UNION ALL SELECT journal_id,item_id,count,color1,color2,color3,bundle_item_count FROM game_item_ledger02").fetch_all(&p).await.unwrap();
    assert!(rows.iter().all(|r| r.0 == 4419625
        && r.1 == -1
        && r.3 == Some(-1)
        && r.4 == Some(-1)
        && r.5 == Some(-1)));
    assert!(rows.iter().any(|r| r.2 == 1 && r.6.is_none()));
    assert!(rows.iter().any(|r| r.2 == 5 && r.6 == Some(5)));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_save_and_prepare_failures_do_not_flush_item_events() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), |_| Err(
            InventoryError::Clock.into()
        ))
        .await,
        Err(DeliveryError::Preparation(_))
    ));
    assert_eq!(count(&p, "items").await, 0);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 1);
    sqlx::query("ALTER TABLE game_items ADD CONSTRAINT reject_delivery CHECK(false) NOT VALID")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared).await,
        Err(DeliveryError::Storage(_))
    ));
    assert_eq!(count(&p, "items").await, 0);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 2);
    let rows:Vec<Vec<u8>>=sqlx::query_scalar("SELECT item_class_utf16 FROM game_item_ledger01 UNION ALL SELECT item_class_utf16 FROM game_item_ledger02").fetch_all(&p).await.unwrap();
    for msg in ["giveDefaultItemFail", "giveDefaultItemFail(save)"] {
        assert!(rows.contains(&msg.encode_utf16().flat_map(u16::to_le_bytes).collect()));
    }
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_logger_errors_do_not_undo_inventory_and_replay_requires_reconciliation() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    for t in [
        "game_item_log01",
        "game_item_log02",
        "game_item_ledger01",
        "game_item_ledger02",
    ] {
        sqlx::query(&format!(
            "ALTER TABLE {t} ADD CONSTRAINT reject_event CHECK(false) NOT VALID"
        ))
        .execute(&p)
        .await
        .unwrap();
    }
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared)
        .await
        .unwrap();
    assert_eq!((o.item_logs.failed, o.ledgers.failed), (4, 3));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 0);
    assert_eq!(count(&p, "ledgers").await, 0);
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), prepared).await,
        Err(DeliveryError::Reconcile)
    ));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_rejects_unloaded_shared_state_and_stale_owner_before_side_effects() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(true), |_| panic!(
            "shared state not loaded"
        ))
        .await,
        Err(DeliveryError::SharedState)
    ));
    assert_eq!(count(&p, "journals").await, 0);
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        delivery::deliver(&p, &fixture::key(), 1, &profile(false), |_| panic!(
            "stale owner"
        ))
        .await,
        Err(DeliveryError::Admission(_))
    ));
    assert_eq!(count(&p, "journals").await, 0);
    assert_eq!(count(&p, "items").await, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_concurrent_attempts_never_duplicate_inventory_or_event_rows() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let profile = profile(false);
    let key = fixture::key();
    let (x, y) = tokio::join!(
        delivery::deliver(&p, &key, 1, &profile, prepared),
        delivery::deliver(&p, &key, 1, &profile, prepared)
    );
    assert!(matches!(
        (&x, &y),
        (Ok(_), Err(DeliveryError::Reconcile)) | (Err(DeliveryError::Reconcile), Ok(_))
    ));
    assert_eq!(count(&p, "items").await, 4);
    assert_eq!(count(&p, "logs").await, 4);
    assert_eq!(count(&p, "ledgers").await, 3);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_delivery_loaded_empty_shared_inventory_records_zero_for_the_last_stack() {
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    use vindictus_server::infrastructure::shared_item_state as shared;
    let raw=br#"{"format_version":1,"source":"study_heroes.shared-items","last_value":null,"items":[],"storage":[]}"#;
    shared::preserve(&p, &shared::read_snapshot(raw.as_slice()).unwrap(), false)
        .await
        .unwrap();
    let o = delivery::deliver(&p, &fixture::key(), 1, &profile(true), prepared)
        .await
        .unwrap();
    assert_eq!((o.item_logs.written, o.ledgers.written), (4, 3));
    let shared:Vec<Option<i32>>=sqlx::query_scalar("SELECT after_shared FROM game_item_log01 UNION ALL SELECT after_shared FROM game_item_log02").fetch_all(&p).await.unwrap();
    assert_eq!(shared.iter().filter(|v| **v == Some(0)).count(), 1);
    assert_eq!(shared.iter().filter(|v| v.is_none()).count(), 3);
    cleanup_game_pool(a, p, s).await;
}

mod appearance_fixture {
    include!("../fixtures/fiona_inventory_support.rs");
    include!("../fixtures/fiona_appearance_support.rs");
}
pub(super) struct AppearanceInputs {
    pub(super) profile: vindictus_server::infrastructure::fiona_appearance::Profile,
    pub(super) inventory: vindictus_server::infrastructure::fiona_inventory_profile::Profile,
    pub(super) catalog: vindictus_server::infrastructure::item_catalog::Package,
    pub(super) colors: vindictus_server::infrastructure::item_colors::Package,
    pub(super) template: vindictus_server::protocol::character_creation::Template<'static>,
    pub(super) stats: vindictus_server::infrastructure::item_stats::Package,
}
impl AppearanceInputs {
    pub(super) fn new() -> Self {
        use vindictus_server::infrastructure::{
            fiona_appearance, fiona_inventory_profile, item_catalog, item_colors,
        };
        let (c, s, h, p) = appearance_fixture::fixtures();
        let bytes = |v: &Value| serde_json::to_vec(v).unwrap();
        Self {
            stats: vindictus_server::infrastructure::item_stats::Package::read(
                bytes(&s).as_slice(),
            )
            .unwrap(),
            profile: fiona_appearance::Profile::read(
                bytes(&appearance_fixture::fixture()).as_slice(),
            )
            .unwrap(),
            inventory: fiona_inventory_profile::Profile::read(bytes(&p).as_slice()).unwrap(),
            catalog: item_catalog::read_package(bytes(&c).as_slice()).unwrap(),
            colors: item_colors::Package::read(bytes(&h).as_slice()).unwrap(),
            template: appearance_fixture::template(),
        }
    }
    pub(super) fn request(
        &self,
    ) -> vindictus_server::infrastructure::fiona_customization::Appearance<'_> {
        vindictus_server::infrastructure::fiona_customization::Appearance {
            profile: &self.profile,
            inventory: &self.inventory,
            catalog: &self.catalog,
            colors: &self.colors,
            template: &self.template,
            arithmetic: None,
        }
    }
}
pub(super) async fn appearance_setup(p: &PgPool, initialized: bool) {
    setup(p).await;
    if initialized {
        use vindictus_server::infrastructure::character_costumes as costumes;
        costumes::preserve(p, &costumes::read_snapshot(br#"{"format_version":1,"source":"study_heroes.costume","source_last_value":null,"rows":[]}"#.as_slice()).unwrap(), false).await.unwrap();
    }
}
async fn costumes_count(p: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_costumes")
        .fetch_one(p)
        .await
        .unwrap()
}
use vindictus_server::infrastructure::fiona_customization::{
    self as customization, CustomizationError,
};
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_saves_every_appearance_field_only_after_inventory_events() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, true).await;
    sqlx::query("CREATE FUNCTION costume_after_events() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN IF (SELECT count(*)FROM game_items)<>4 OR (SELECT count(*)FROM game_item_log01)+(SELECT count(*)FROM game_item_log02)<>4 OR (SELECT count(*)FROM game_item_ledger01)+(SELECT count(*)FROM game_item_ledger02)<>3 THEN RAISE EXCEPTION 'appearance before inventory and events'; END IF; RETURN NEW; END$$").execute(&p).await.unwrap();
    sqlx::query("CREATE TRIGGER costume_after_events BEFORE INSERT ON game_costumes FOR EACH ROW EXECUTE FUNCTION costume_after_events()").execute(&p).await.unwrap();
    let inputs = AppearanceInputs::new();
    let outcome = customization::customize(
        &p,
        &fixture::key(),
        1,
        &profile(false),
        inputs.request(),
        prepared,
    )
    .await
    .unwrap();
    assert_eq!(
        (
            outcome.costumes_changed,
            outcome.appearance.len(),
            outcome.inventory.item_logs.written
        ),
        (10, 10, 4)
    );
    assert_eq!(outcome.gear.rows().len(), 1);
    assert_eq!(
        outcome
            .gear
            .item_stats(&vindictus_server::domain::skill_runtime::Cache::default())
            .unwrap()
            .armor_hp
            .get(&4),
        Some(&0)
    );
    let assigned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_equippables WHERE part IS NOT NULL")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(assigned, 0, "equippable type does not imply equipped Part");
    use vindictus_server::domain::character_costume::Costume;
    #[derive(sqlx::FromRow)]
    struct Row {
        is_base: bool,
        costume_type: i16,
        costume_key: i16,
        costume_sn: Option<i32>,
        c1: Option<i32>,
        c2: Option<i32>,
        c3: Option<i32>,
        gloss: Option<i16>,
        effect: Option<i32>,
    }
    let stored: Vec<Row> = sqlx::query_as("SELECT is_base,costume_type,costume_key,costume_sn,c1,c2,c3,gloss,effect FROM game_costumes WHERE cid=1 ORDER BY row_id").fetch_all(&p).await.unwrap();
    let actual = stored
        .into_iter()
        .map(|r| Costume {
            is_base: r.is_base,
            kind: r.costume_type.try_into().unwrap(),
            key: r.costume_key.try_into().unwrap(),
            serial: r.costume_sn,
            colors: [r.c1, r.c2, r.c3],
            gloss: r.gloss.map(|v| v.try_into().unwrap()),
            effect: r.effect,
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, outcome.appearance);
    assert_eq!(actual[0].colors, [Some(7), Some(8), Some(9)]);
    assert_eq!(actual[1].colors, [Some(10), Some(30), Some(20)]);
    assert_eq!(actual[9].serial, Some(500));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_invalid_appearance_keeps_inventory_and_refuses_replay() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, true).await;
    let mut inputs = AppearanceInputs::new();
    inputs.template.eye_color = -999;
    assert!(matches!(
        customization::customize(
            &p,
            &fixture::key(),
            1,
            &profile(false),
            inputs.request(),
            prepared
        )
        .await,
        Err(CustomizationError::Appearance(_))
    ));
    assert_eq!(
        (
            count(&p, "items").await,
            count(&p, "logs").await,
            count(&p, "ledgers").await,
            costumes_count(&p).await
        ),
        (4, 4, 3, 0)
    );
    inputs.template.eye_color = 20;
    assert!(matches!(
        customization::customize(
            &p,
            &fixture::key(),
            1,
            &profile(false),
            inputs.request(),
            prepared
        )
        .await,
        Err(CustomizationError::Inventory(DeliveryError::Reconcile))
    ));
    assert_eq!(
        (
            count(&p, "logs").await,
            count(&p, "ledgers").await,
            costumes_count(&p).await
        ),
        (4, 3, 0)
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_late_costume_failure_rolls_back_only_appearance() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, true).await;
    sqlx::query(
        "ALTER TABLE game_costumes ADD CONSTRAINT reject_late_appearance CHECK(costume_type<>3)",
    )
    .execute(&p)
    .await
    .unwrap();
    let inputs = AppearanceInputs::new();
    assert!(matches!(
        customization::customize(
            &p,
            &fixture::key(),
            1,
            &profile(false),
            inputs.request(),
            prepared
        )
        .await,
        Err(CustomizationError::Storage(_))
    ));
    assert_eq!(
        (
            count(&p, "items").await,
            count(&p, "logs").await,
            count(&p, "ledgers").await,
            costumes_count(&p).await
        ),
        (4, 4, 3, 0)
    );
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_requires_preserved_identity_before_costume_write() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, false).await;
    let inputs = AppearanceInputs::new();
    assert!(matches!(
        customization::customize(
            &p,
            &fixture::key(),
            1,
            &profile(false),
            inputs.request(),
            prepared
        )
        .await,
        Err(CustomizationError::Storage(
            vindictus_server::infrastructure::character_costumes::CostumeError::Uninitialized
        ))
    ));
    assert_eq!((count(&p, "items").await, costumes_count(&p).await), (4, 0));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_rechecks_revision_and_first_fiona_after_inventory_commit() {
    for mutation in [
        "UPDATE game_users SET service_id=service_id WHERE id=31",
        "UPDATE game_character_info SET class=2 WHERE id=1",
    ] {
        let (a, p, s) = isolated_game_pool().await;
        appearance_setup(&p, true).await;
        sqlx::query(&format!("CREATE FUNCTION change_owner_at_ledger() RETURNS trigger LANGUAGE plpgsql AS $$BEGIN {mutation}; RETURN NEW; END$$")).execute(&p).await.unwrap();
        for t in ["game_item_ledger01", "game_item_ledger02"] {
            sqlx::query(&format!("CREATE TRIGGER change_owner_at_ledger AFTER INSERT ON {t} FOR EACH ROW EXECUTE FUNCTION change_owner_at_ledger()")).execute(&p).await.unwrap();
        }
        let inputs = AppearanceInputs::new();
        assert!(matches!(
            customization::customize(
                &p,
                &fixture::key(),
                1,
                &profile(false),
                inputs.request(),
                prepared
            )
            .await,
            Err(CustomizationError::Storage(
                vindictus_server::infrastructure::character_costumes::CostumeError::Admission(_)
            ))
        ));
        assert_eq!((count(&p, "items").await, costumes_count(&p).await), (4, 0));
        cleanup_game_pool(a, p, s).await;
    }
}
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_race_writes_one_inventory_event_and_appearance_set() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, true).await;
    let inputs = AppearanceInputs::new();
    let profile = profile(false);
    let key = fixture::key();
    let (x, y) = tokio::join!(
        customization::customize(&p, &key, 1, &profile, inputs.request(), prepared),
        customization::customize(&p, &key, 1, &profile, inputs.request(), prepared)
    );
    assert_eq!(usize::from(x.is_ok()) + usize::from(y.is_ok()), 1);
    assert!(matches!(
        x.err().or(y.err()).unwrap(),
        CustomizationError::Inventory(DeliveryError::Reconcile)
    ));
    assert_eq!(
        (
            count(&p, "items").await,
            count(&p, "logs").await,
            count(&p, "ledgers").await,
            costumes_count(&p).await
        ),
        (4, 4, 3, 10)
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL"]
async fn fiona_customization_refuses_invalid_slot_metadata_before_inventory_effects() {
    let (a, p, s) = isolated_game_pool().await;
    appearance_setup(&p, true).await;
    let mut inputs = AppearanceInputs::new();
    let mut v = appearance_fixture::fixture();
    let duplicate = v["parts"][0].clone();
    v["parts"].as_array_mut().unwrap().push(duplicate);
    inputs.profile = vindictus_server::infrastructure::fiona_appearance::Profile::read(
        serde_json::to_vec(&v).unwrap().as_slice(),
    )
    .unwrap();
    assert!(matches!(
        customization::customize(
            &p,
            &fixture::key(),
            1,
            &profile(false),
            inputs.request(),
            |_| panic!("delivery must not start")
        )
        .await,
        Err(CustomizationError::Binding(_))
    ));
    assert_eq!(
        (
            count(&p, "items").await,
            count(&p, "logs").await,
            count(&p, "ledgers").await,
            costumes_count(&p).await
        ),
        (0, 0, 0, 0)
    );
    cleanup_game_pool(a, p, s).await;
}

mod loader_fixture {
    include!("../fixtures/item_loader_support.rs");
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_gear_proves_both_associations_before_empty_projection_without_writes() {
    use vindictus_server::infrastructure::item_loader_state as loader;
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let input = AppearanceInputs::new();
    assert!(matches!(
        loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog).await,
        Err(loader::LoaderError::Uninitialized)
    ));
    let snap = loader::read_snapshot(
        serde_json::to_vec(&loader_fixture::empty())
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    loader::preserve(&p, &snap, false).await.unwrap();
    let seq: (i64, bool) = sqlx::query_as("SELECT last_value,is_called FROM game_quick_slot_id")
        .fetch_one(&p)
        .await
        .unwrap();
    let out = loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog)
        .await
        .unwrap();
    assert_eq!(out.gear_slots(), 1);
    sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,tradable)VALUES(77,$1,1,1,0,0,true)").bind(vec![65u8,0]).execute(&p).await.unwrap();
    assert!(matches!(
        loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog).await,
        Err(loader::LoaderError::Nonempty)
    ));
    sqlx::query("DELETE FROM game_items WHERE id=77")
        .execute(&p)
        .await
        .unwrap();

    let after: (i64, bool) = sqlx::query_as("SELECT last_value,is_called FROM game_quick_slot_id")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(seq, after);
    assert_eq!(count(&p, "items").await, 0);
    assert_eq!(count(&p, "logs").await, 0);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn item_loader_gear_refuses_nonempty_associations_stale_owner_and_changed_content() {
    use vindictus_server::infrastructure::item_loader_state as loader;
    let (a, p, s) = isolated_game_pool().await;
    setup(&p).await;
    let mut input = AppearanceInputs::new();
    let snap = loader::read_snapshot(
        serde_json::to_vec(&loader_fixture::empty())
            .unwrap()
            .as_slice(),
    )
    .unwrap();
    loader::preserve(&p, &snap, false).await.unwrap();
    input.catalog.manifest.config_sha256 = "d".repeat(64);
    assert!(matches!(
        loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog).await,
        Err(loader::LoaderError::Content)
    ));
    input = AppearanceInputs::new();
    sqlx::query("INSERT INTO game_quick_slots(owner_id,item_class_utf16,slot)VALUES(1,$1,0)")
        .bind(vec![65u8, 0])
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog).await,
        Err(loader::LoaderError::Nonempty)
    ));
    sqlx::query("UPDATE game_users SET service_id=service_id WHERE id=31")
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        loader::load_initial_gear(&p, &fixture::key(), 1, &input.profile, &input.catalog).await,
        Err(loader::LoaderError::Owner(state::StateError::Ownership))
    ));
    assert_eq!(count(&p, "items").await, 0);
    cleanup_game_pool(a, p, s).await;
}

pub(super) fn initial_appearance_inputs() -> (
    vindictus_server::infrastructure::fiona_appearance::Profile,
    vindictus_server::infrastructure::item_catalog::Package,
) {
    let i = AppearanceInputs::new();
    (i.profile, i.catalog)
}
