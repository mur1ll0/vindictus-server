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
    // Event switches (migration 0057) reach the overlay; the newest edit of a key wins.
    for (enabled, n) in [(true, 5), (false, 6)] {
        let rows = [(
            "event",
            "Event_Christmas_2010".to_owned(),
            Some(serde_json::json!({ "enabled": enabled })),
        )];
        assert_eq!(
            content_edits::publish_now(&pool, chefe.account_id, &rows, audit(Uuid::new_v4()))
                .await
                .unwrap(),
            content_edits::EditOutcome::Done(n)
        );
    }
    let loaded = content_edits::loaded_overlay(&pool).await.unwrap();
    assert_eq!(loaded.events.get("Event_Christmas_2010"), Some(&false));
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
        content_edits::EditOutcome::Done(7)
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

/// Item editor (docs/future/item-editor.md, cut 2): a new item from a base template, a base
/// override with only what changed, hide, the refusals, publishing and the overlay's item parts.
#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn item_editor_edits_reach_the_overlay() {
    use vindictus_server::{
        application::item_editor::{self, ItemDraft, Sources},
        infrastructure::admin_characters::EnglishNames,
    };
    let (admin_pool, pool, schema) = isolated_pool().await;
    create_account(
        &pool,
        Username::parse("Chefe").unwrap(),
        Zeroizing::new(PASSWORD.as_bytes().to_vec()),
    )
    .await
    .unwrap();
    set_role_from_cli(&pool, "Chefe", Some(PanelRole::Admin))
        .await
        .unwrap();
    let chefe = PanelAuth::new(pool.clone())
        .login("chefe", Zeroizing::new(PASSWORD.as_bytes().to_vec()), None)
        .await
        .unwrap()
        .session;
    let names = EnglishNames::read(
        serde_json::json!({"items": {"FIONA_INNER_FRINGE_SOCKS": ["Frilled Inner Armor", "Socks"]}})
            .to_string()
            .as_bytes(),
    )
    .unwrap()
    .with_catalog(
        serde_json::json!({
            "item_icons": {"FIONA_INNER_FRINGE_SOCKS": "fiona_inner_fringe_socks"},
            "item_info": {"FIONA_INNER_FRINGE_SOCKS": {"c": "INNERARMOR", "set": "fiona_set"}},
            "item_edit": {
                "FIONA_INNER_FRINGE_SOCKS": {"o": "b", "f": {"Icon": "fiona_inner_fringe_socks",
                    "Category": "INNERARMOR", "CostumeType": 24, "DEF": 10}},
                "FIONA_INNER_LAB": {"o": "a", "f": {"Category": "INNERARMOR", "CostumeType": 9101},
                    "d": {"ItemClass": "fiona_inner_lab", "template": "fiona_inner_fringe_socks"}}
            }
        })
        .to_string()
        .as_bytes(),
    )
    .unwrap();
    let sources = Sources {
        names: &names,
        client: None,
        assets: None,
    };
    let actor = |op| Actor {
        session: &chefe,
        peer: None,
        operation_id: op,
    };
    let draft = |class: &str, template: Option<&str>, fields: serde_json::Value| ItemDraft {
        item_class: class.into(),
        template: template.map(str::to_owned),
        name: "Lab Inner".into(),
        description: Some("Panel item".into()),
        fields: fields.as_object().unwrap().clone(),
        set_id: None,
        kit: None,
        body: None,
        restore_model: false,
    };
    // New item from a base template.
    let saved = item_editor::save(
        &pool,
        &actor(Uuid::new_v4()),
        &sources,
        &draft(
            "fiona_inner_new",
            Some("fiona_inner_fringe_socks"),
            serde_json::json!({"DEF": 40, "Icon": "fiona_inner_fringe_socks"}),
        ),
    )
    .await
    .unwrap();
    assert_eq!(saved.kind, "item");
    // Refusals: an existing class, a template that is an added item, a column outside the
    // editor, an unknown icon, removing a base item.
    for (bad, why) in [
        (
            draft(
                "fiona_inner_lab",
                Some("fiona_inner_fringe_socks"),
                serde_json::json!({}),
            ),
            "exists",
        ),
        (
            draft(
                "fiona_inner_x",
                Some("fiona_inner_lab"),
                serde_json::json!({}),
            ),
            "template",
        ),
        (
            draft(
                "fiona_inner_y",
                Some("fiona_inner_fringe_socks"),
                serde_json::json!({"Feature": "x"}),
            ),
            "column",
        ),
        (
            draft(
                "fiona_inner_z",
                Some("fiona_inner_fringe_socks"),
                serde_json::json!({"Icon": "nope"}),
            ),
            "icon",
        ),
    ] {
        let refused = item_editor::save(&pool, &actor(Uuid::new_v4()), &sources, &bad).await;
        assert!(
            matches!(
                refused,
                Err(AdminError::InvalidInput | AdminError::Unchanged(_))
            ),
            "{why}"
        );
    }
    assert!(matches!(
        item_editor::remove(
            &pool,
            &actor(Uuid::new_v4()),
            &names,
            "fiona_inner_fringe_socks"
        )
        .await,
        Err(AdminError::Unchanged("item_base"))
    ));
    // Base item: only the changed column and the new name go into the override.
    let mut base = draft(
        "fiona_inner_fringe_socks",
        None,
        serde_json::json!({"DEF": 15, "CostumeType": 24, "Category": "INNERARMOR", "Icon": "fiona_inner_fringe_socks"}),
    );
    base.set_id = Some("fiona_set".into());
    base.description = Some("Socks".into());
    assert_eq!(
        item_editor::save(&pool, &actor(Uuid::new_v4()), &sources, &base)
            .await
            .unwrap()
            .kind,
        "item_override"
    );
    item_editor::hide(
        &pool,
        &actor(Uuid::new_v4()),
        &names,
        "fiona_inner_lab",
        true,
    )
    .await
    .unwrap();
    item_editor::remove(&pool, &actor(Uuid::new_v4()), &names, "fiona_inner_lab")
        .await
        .unwrap();
    let listed = item_editor::list(&pool, &chefe, &names, "", Some("pending"), (None, None))
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert!(
        listed
            .iter()
            .any(|i| i.item_class == "fiona_inner_new" && !i.exported)
    );

    let (version, _) = content_admin::publish(&pool, &actor(Uuid::new_v4()), None, 0)
        .await
        .unwrap();
    assert_eq!(
        i64::from(content_edits::latest_item_version(&pool).await.unwrap()),
        version
    );
    let overlay = content_edits::overlay(&pool).await.unwrap();
    assert_eq!(overlay["items"][0]["ItemClass"], "fiona_inner_new");
    assert_eq!(overlay["items"][0]["template"], "fiona_inner_fringe_socks");
    assert_eq!(overlay["items"][0]["set"]["EquipItemInfo"]["DEF"], 40);
    assert_eq!(
        overlay["item_overrides"]["fiona_inner_fringe_socks"],
        serde_json::json!({"set": {"EquipItemInfo": {"DEF": 15}}, "text": {"name": "Lab Inner"}})
    );
    // Removing also drops the hidden mark of the same item.
    assert_eq!(overlay["hidden_items"], serde_json::json!([]));
    assert_eq!(
        overlay["removed_items"],
        serde_json::json!(["fiona_inner_lab"])
    );
    content_edits::loaded_overlay(&pool).await.unwrap();
    let detail = item_editor::detail(&pool, &chefe, &names, "fiona_inner_new")
        .await
        .unwrap();
    assert_eq!((detail.origin.as_str(), detail.usage.total), ("added", 0));
    // Restoring the base item publishes as an empty override: the overlay drops it.
    item_editor::restore(
        &pool,
        &actor(Uuid::new_v4()),
        &names,
        "fiona_inner_fringe_socks",
    )
    .await
    .unwrap();
    content_admin::publish(&pool, &actor(Uuid::new_v4()), None, 0)
        .await
        .unwrap();
    let overlay = content_edits::overlay(&pool).await.unwrap();
    assert_eq!(overlay["item_overrides"], serde_json::json!({}));

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;
}

#[tokio::test]
#[ignore = "requires TEST_DATABASE_URL pointing to disposable PostgreSQL"]
async fn event_schedule_registers_runs_and_removes_events() {
    use vindictus_server::{
        domain::event_schedule::State,
        infrastructure::event_scheduler::{self as scheduler, RegisterError, Registration},
    };
    let (admin_pool, pool, schema) = isolated_pool().await;
    create_account(
        &pool,
        Username::parse("Agenda").unwrap(),
        Zeroizing::new(PASSWORD.as_bytes().to_vec()),
    )
    .await
    .unwrap();
    let account: Uuid = sqlx::query_scalar("SELECT id FROM accounts LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let state = std::sync::Mutex::new(State::default());
    let templates = |name: &str| {
        (name == "#exp2x").then(|| {
            (
                Some("ExpRate_Combat[200];Event_xmas".to_owned()),
                Some("droptable mod gold_ore 300;monster_refresh;".to_owned()),
                Some("droptable reset gold_ore;".to_owned()),
            )
        })
    };
    let now = 1_793_700_000;
    let reg =
        |name: &str, template: Option<&str>, start: Option<i64>, end: Option<i64>| Registration {
            name: name.to_owned(),
            template: template.map(str::to_owned),
            start_at: start,
            end_at: end,
            start_message: Some("started".into()),
            ..Registration::default()
        };
    // Refusals: an unknown template, a start in the past, an empty name.
    for (r, e) in [
        (
            reg("x", Some("#nope"), None, None),
            RegisterError::NoTemplate,
        ),
        (reg("x", None, Some(now - 60), None), RegisterError::Time),
        (reg(" ", None, None, None), RegisterError::Invalid),
    ] {
        assert_eq!(
            scheduler::register(&pool, &state, &templates, r, account, now)
                .await
                .unwrap(),
            Err(e)
        );
    }
    // The template fills Feature and the scripts; it runs from now + 60 to now + 3600.
    scheduler::register(
        &pool,
        &state,
        &templates,
        reg("exp", Some("#exp2x"), Some(now + 60), Some(now + 3600)),
        account,
        now,
    )
    .await
    .unwrap()
    .unwrap();
    let pass = scheduler::process(&pool, &state, now, 0).await.unwrap();
    assert!(!pass.reload && pass.notices.is_empty(), "not started yet");
    let pass = scheduler::process(&pool, &state, now + 120, 0)
        .await
        .unwrap();
    assert!(pass.reload);
    assert_eq!(pass.notices, ["started"]);
    {
        let s = state.lock().unwrap();
        assert_eq!(
            s.overrides.get("ExpRate_Combat").map(String::as_str),
            Some("200")
        );
        assert_eq!(s.drop_modifiers.get("gold_ore"), Some(&300));
    }
    let listed = scheduler::list(&pool, &state).await.unwrap();
    assert!(listed[0].running);
    assert_eq!(
        listed[0].registration.feature.as_deref(),
        Some("ExpRate_Combat[200];Event_xmas")
    );
    // A running event cannot change.
    assert_eq!(
        scheduler::register(
            &pool,
            &state,
            &templates,
            reg("exp", None, None, None),
            account,
            now
        )
        .await
        .unwrap(),
        Err(RegisterError::Running)
    );
    // Removed from the table: it ends at the next pass and everything goes back.
    assert!(scheduler::remove(&pool, "exp").await.unwrap());
    let pass = scheduler::process(&pool, &state, now + 180, 0)
        .await
        .unwrap();
    assert!(pass.reload);
    assert!(state.lock().unwrap().overrides.is_empty());
    // Past its EndTime an event is deleted.
    scheduler::register(
        &pool,
        &state,
        &templates,
        reg("short", None, None, Some(now + 600)),
        account,
        now,
    )
    .await
    .unwrap()
    .unwrap();
    scheduler::process(&pool, &state, now + 300, 0)
        .await
        .unwrap();
    assert!(state.lock().unwrap().running.contains_key("short"));
    scheduler::process(&pool, &state, now + 600, 0)
        .await
        .unwrap();
    assert!(scheduler::list(&pool, &state).await.unwrap().is_empty());
    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA {schema} CASCADE"))
        .execute(&admin_pool)
        .await
        .unwrap();
    admin_pool.close().await;
}
