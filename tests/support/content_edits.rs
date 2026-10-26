//! Content edits (B4.4) against PostgreSQL: pending → published versions, withdrawal, the
//! cumulative overlay (a later edit of a target wins), the applied mark, repeated operation
//! ids and who may publish. Part of the postgres test target; needs TEST_DATABASE_URL.
use super::admin_panel::isolated_pool;
use uuid::Uuid;
use vindictus_server::{
    application::{
        accounts::create_account,
        admin_session::{AdminError, PanelAuth, set_role_from_cli},
        content_admin::{self, Actor},
    },
    domain::{Username, admin::PanelRole},
    infrastructure::{
        admin_store::AuditRecord,
        content_edits::{self, EditOutcome},
    },
};
use zeroize::Zeroizing;

const PASSWORD: &str = "senha de teste do painel";

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn content_edits_publish_versions_and_export_the_cumulative_overlay() {
    let (admin_pool, pool, schema) = isolated_pool().await;
    for (name, role) in [("Chefe", PanelRole::Admin), ("Ajudante", PanelRole::Gm)] {
        create_account(
            &pool,
            Username::parse(name).unwrap(),
            Zeroizing::new(PASSWORD.as_bytes().to_vec()),
        )
        .await
        .unwrap();
        set_role_from_cli(&pool, name, Some(role)).await.unwrap();
    }
    let auth = PanelAuth::new(pool.clone());
    let chefe = auth
        .login("chefe", Zeroizing::new(PASSWORD.as_bytes().to_vec()), None)
        .await
        .unwrap()
        .session;
    let gm = auth
        .login(
            "ajudante",
            Zeroizing::new(PASSWORD.as_bytes().to_vec()),
            None,
        )
        .await
        .unwrap()
        .session;
    let audit = |op: Uuid| AuditRecord {
        account_id: Some(gm.account_id),
        actor_name: Some("Ajudante"),
        role: gm.role,
        operation_id: Some(op),
        action: "content.edit",
        target_kind: Some("content"),
        target_id: Some("x"),
        before: None,
        after: None,
        refused: false,
        peer: None,
    };
    let edit =
        |kind: &'static str, target: &'static str, value: Option<serde_json::Value>, op: Uuid| {
            let pool = pool.clone();
            let author = gm.account_id;
            let record = audit(op);
            async move {
                content_edits::create(&pool, kind, target, value.as_ref(), author, record)
                    .await
                    .unwrap()
            }
        };
    let row = |price: i64| {
        Some(
            serde_json::json!({"ItemClass":"enhance_elixir","Count":1,"PriceItemClass":"gold","PriceCount":price}),
        )
    };
    let first = Uuid::new_v4();
    let EditOutcome::Done(a) = edit("shop_row", "FERGHUS_Shop#124", row(3000), first).await else {
        panic!()
    };
    assert_eq!(
        edit("shop_row", "FERGHUS_Shop#124", row(9), first).await,
        EditOutcome::Repeated
    );
    edit(
        "cash_price",
        "HAIR",
        Some(serde_json::json!({"price": 900})),
        Uuid::new_v4(),
    )
    .await;
    let EditOutcome::Done(withdrawn) = edit(
        "cash_default",
        "default",
        Some(serde_json::json!({"price": 1})),
        Uuid::new_v4(),
    )
    .await
    else {
        panic!()
    };
    let actor = |session| Actor {
        session,
        peer: None,
        operation_id: Uuid::new_v4(),
    };
    content_admin::withdraw(&pool, &actor(&gm), withdrawn)
        .await
        .unwrap();
    assert!(matches!(
        content_admin::withdraw(&pool, &actor(&gm), withdrawn).await,
        Err(AdminError::NotFound)
    ));

    // Only an admin publishes; the pending edits become version 1 with a restart in 5
    // minutes. While it is scheduled no other publish; cancelling is audited once.
    let maintenance = vindictus_server::infrastructure::maintenance::Maintenance::default();
    assert!(matches!(
        content_admin::publish(&pool, &actor(&gm), Some(&maintenance), 5).await,
        Err(AdminError::Forbidden)
    ));
    assert!(matches!(
        content_admin::publish(&pool, &actor(&chefe), Some(&maintenance), 61).await,
        Err(AdminError::InvalidInput)
    ));
    let (version, plan) = content_admin::publish(&pool, &actor(&chefe), Some(&maintenance), 5)
        .await
        .unwrap();
    let plan = plan.unwrap();
    assert_eq!((version, plan.version, plan.delay_seconds), (1, 1, 300));
    assert!(matches!(
        content_admin::publish(&pool, &actor(&chefe), Some(&maintenance), 0).await,
        Err(AdminError::Unchanged("manutencao"))
    ));
    let cancel = actor(&chefe);
    content_admin::cancel_maintenance(&pool, &cancel, &maintenance)
        .await
        .unwrap();
    assert!(maintenance.current().is_none());
    assert!(matches!(
        content_admin::cancel_maintenance(&pool, &cancel, &maintenance).await,
        Err(AdminError::NotFound)
    ));
    assert!(matches!(
        content_admin::publish(&pool, &actor(&chefe), Some(&maintenance), 0).await,
        Err(AdminError::Unchanged("sem_pendentes"))
    ));
    // A later edit of the same row (version 2) wins in the cumulative overlay; a removal too.
    edit("shop_row", "FERGHUS_Shop#124", row(5000), Uuid::new_v4()).await;
    edit("shop_row_remove", "FERGHUS_Shop#3", None, Uuid::new_v4()).await;
    assert_eq!(
        content_admin::publish(&pool, &actor(&chefe), None, 0)
            .await
            .unwrap()
            .0,
        2
    );
    let overlay = content_edits::overlay(&pool).await.unwrap();
    assert_eq!(overlay["version"], 2);
    assert_eq!(overlay["cash_prices"]["HAIR"], 900);
    assert!(overlay["cash_default"].is_null());
    let rows = overlay["shop_rows"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let elixir = rows.iter().find(|r| r["Order"] == 124).unwrap();
    assert_eq!(
        (elixir["ShopID"].as_str(), elixir["PriceCount"].as_i64()),
        (Some("FERGHUS_Shop"), Some(5000))
    );
    assert_eq!(
        rows.iter().find(|r| r["Order"] == 3).unwrap()["Remove"],
        true
    );
    // The game server applies the same overlay, bounds checked, when it starts.
    let loaded = content_edits::loaded_overlay(&pool).await.unwrap();
    assert_eq!((loaded.version, loaded.shop_rows.len()), (2, 2));
    assert_eq!(loaded.cash_prices.get("HAIR"), Some(&900));
    assert!(content_edits::mark_applied(&pool, 2).await.unwrap());
    assert!(!content_edits::mark_applied(&pool, 2).await.unwrap());
    let (edits, versions) = content_admin::edits(&pool, &gm).await.unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions.iter().all(|v| v.applied_at.is_some()));
    assert_eq!(edits.iter().filter(|e| e.state == "withdrawn").count(), 1);
    assert_eq!(edits.iter().find(|e| e.id == a).unwrap().version, Some(1));

    // B4.5 tuning kinds (migration 0047) reach the loaded overlay, newest edit per target.
    edit(
        "feature",
        "ExpRate_Combat",
        Some(serde_json::json!({"value": 200})),
        Uuid::new_v4(),
    )
    .await;
    edit(
        "quest_drop",
        "QUEST_A",
        Some(serde_json::json!({"min": 3, "max": 5})),
        Uuid::new_v4(),
    )
    .await;
    edit(
        "enhance_ratio",
        "5#5",
        Some(serde_json::json!({"ratio": 0.9})),
        Uuid::new_v4(),
    )
    .await;
    edit(
        "enchant_ratio",
        "simple",
        Some(serde_json::json!({"min": 40, "max": 80})),
        Uuid::new_v4(),
    )
    .await;
    edit(
        "feature",
        "ExpRate_Combat",
        Some(serde_json::json!({"value": 300})),
        Uuid::new_v4(),
    )
    .await;
    assert_eq!(
        content_admin::publish(&pool, &actor(&chefe), None, 0)
            .await
            .unwrap()
            .0,
        3
    );
    let loaded = content_edits::loaded_overlay(&pool).await.unwrap();
    assert_eq!(loaded.features.get("ExpRate_Combat"), Some(&300));
    assert_eq!(loaded.quest_drops.get("QUEST_A"), Some(&[3, 5]));
    assert_eq!(loaded.enhance_ratios.get("5#5"), Some(&0.9));
    assert_eq!(loaded.enchant_ratios.get("simple"), Some(&[40, 80]));

    // B4.5b: a live tuning enters published as its own version; pending edits stay pending
    // and a repeated operation is refused.
    edit(
        "feature",
        "GoldRate_Story",
        Some(serde_json::json!({"value": 150})),
        Uuid::new_v4(),
    )
    .await;
    let op = Uuid::new_v4();
    let rows = [(
        "feature",
        "ExpRate_Combat".to_owned(),
        Some(serde_json::json!({"value": 500})),
    )];
    assert_eq!(
        content_edits::publish_now(&pool, chefe.account_id, &rows, audit(op))
            .await
            .unwrap(),
        content_edits::EditOutcome::Done(4)
    );
    assert_eq!(
        content_edits::publish_now(&pool, chefe.account_id, &rows, audit(op))
            .await
            .unwrap(),
        content_edits::EditOutcome::Repeated
    );
    let loaded = content_edits::loaded_overlay(&pool).await.unwrap();
    assert_eq!(loaded.features.get("ExpRate_Combat"), Some(&500));
    assert_eq!(loaded.features.get("GoldRate_Story"), None);
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content_edits WHERE state = 'pending'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending, 1);
    // Saving the same value live withdraws its pending edit, so a later publish keeps it.
    let rows = [(
        "feature",
        "GoldRate_Story".to_owned(),
        Some(serde_json::json!({"value": 120})),
    )];
    assert_eq!(
        content_edits::publish_now(&pool, chefe.account_id, &rows, audit(Uuid::new_v4()))
            .await
            .unwrap(),
        content_edits::EditOutcome::Done(5)
    );
    let pending: i64 =
        sqlx::query_scalar("SELECT count(*) FROM content_edits WHERE state = 'pending'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pending, 0);
    content_edits::mark_version_applied(&pool, 4).await.unwrap();

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;
}
