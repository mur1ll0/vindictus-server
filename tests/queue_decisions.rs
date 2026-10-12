use serde_json::Value;
use vindictus_server::domain::{
    character_data::QueuedItemSnapshot,
    legacy_user_data::{LegacyBytes, SqlDateTime, SqlDateTime2},
    queued_item::{
        QueueBinder, QueueMailType, QueueRoute, attempt_decision, decide, default_character_binder,
        selected,
    },
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-queue-decisions.json")).unwrap()
}
fn bytes<const N: usize>(value: &Value) -> Option<LegacyBytes<N, true>> {
    value.as_str().map(|text| {
        LegacyBytes::from_bytes(
            (0..text.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
                .collect(),
        )
        .unwrap()
    })
}
fn row() -> QueuedItemSnapshot {
    QueuedItemSnapshot {
        row_id: 1,
        cid: 7,
        item_class_ex: LegacyBytes::from_bytes(vec![]).unwrap(),
        count: -1,
        is_character_binded: false,
        mail_title: None,
        mail_content: None,
        color1: -1,
        color2: -1,
        color3: -1,
        reduced_durability: 0,
        max_durability_bonus: 0,
        item_id: None,
        processed: false,
        event_code: None,
        processed_time: None,
        expire_time: None,
        log_date: None,
    }
}

#[test]
fn refresh_gate_and_event_count_match_recovered_loop_even_after_failure() {
    let fixture = fixture();
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["attempts"].as_array().unwrap() {
            let event = bytes::<50>(&case["event_hex"]);
            let decision = attempt_decision(
                event.as_ref(),
                case["festival"].as_bool().unwrap(),
                case["refresh"].as_bool().unwrap(),
            );
            assert_eq!(decision.should_attempt, case["attempted"] == 1, "{case}");
            assert_eq!(decision.count_as_event, case["event_count"] == 1, "{case}");
            // The fixture recovers the outer catch; this port does not execute delivery.
            assert_eq!(
                case["errors"] == 1,
                decision.should_attempt && case["throws"] == true
            );
        }
    }
}
#[test]
fn routes_match_source_predicates_with_null_empty_opaque_and_datetime_boundaries() {
    let fixture = fixture();
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["routes"].as_array().unwrap() {
            let mut row = row();
            row.event_code = bytes(&case["event_hex"]);
            row.item_id = case["item_id"].as_i64();
            row.mail_title = bytes(&case["title_hex"]);
            row.mail_content = bytes(&case["content_hex"]);
            row.expire_time = case["expire_units"]
                .as_str()
                .map(|s| SqlDateTime::from_units300(s.parse().unwrap()).unwrap());
            let now =
                SqlDateTime2::from_ticks(case["now_ticks"].as_str().unwrap().parse().unwrap())
                    .unwrap();
            let decision = decide(&row, now).unwrap();
            let mail = match decision.mail_type {
                QueueMailType::Gm => "GMMail",
                QueueMailType::GuildLevelUp => "GuildLevelUpItemMail",
                QueueMailType::Auction => "AuctionItemMail",
                QueueMailType::Dungeon => "DungeonItemMail",
            };
            let binder = match decision.binder {
                QueueBinder::Default => "Default",
                QueueBinder::MobileApp => "MobileApp",
            };
            let route = match decision.route {
                QueueRoute::Expired { system_item_id } => {
                    assert_eq!(system_item_id, row.item_id);
                    "Expired"
                }
                QueueRoute::CreateToMail => "CreateToMail",
                QueueRoute::CreateToInventory => "CreateToInventory",
                QueueRoute::ExistingToMail { item_id } => {
                    assert_eq!(Some(item_id), row.item_id);
                    "ExistingToMail"
                }
                QueueRoute::ExistingToInventory { item_id } => {
                    assert_eq!(Some(item_id), row.item_id);
                    "ExistingToInventory"
                }
            };
            assert_eq!(mail, case["expected"]["mail_type"], "{case}");
            assert_eq!(binder, case["expected"]["binder"], "{case}");
            assert_eq!(route, case["expected"]["route"], "{case}");
        }
    }
}
#[test]
fn default_binding_matches_source_short_circuit_currency_override_and_errors() {
    let fixture = fixture();
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["binds"].as_array().unwrap() {
            let item = bytes::<1024>(&case["item_hex"]).unwrap();
            let mut calls = 0;
            let result = default_character_binder(case["bound"].as_bool().unwrap(), &item, || {
                calls += 1;
                if case["throws"] == true {
                    Err(())
                } else {
                    Ok(case["feature"].as_bool().unwrap())
                }
            });
            assert_eq!(calls, case["feature_calls"].as_i64().unwrap(), "{case}");
            if case["error"] == true {
                assert_eq!(result, Err(()), "{case}");
            } else {
                assert_eq!(result, Ok(case["result"].as_bool().unwrap()), "{case}");
            }
        }
    }
}
#[test]
fn queue_selection_matches_owner_and_processed_without_other_filters() {
    let fixture = fixture();
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["selection"].as_array().unwrap() {
            let mut row = row();
            row.cid = case["cid"].as_str().unwrap().parse().unwrap();
            row.processed = case["processed"].as_bool().unwrap();
            // Expired, orphan system item and already stamped rows still enter source selection.
            row.item_id = Some(i64::MIN);
            row.expire_time = Some(SqlDateTime::from_units300(SqlDateTime::MIN).unwrap());
            row.processed_time = row.expire_time;
            assert_eq!(
                selected(&row, 7),
                case["selected"].as_bool().unwrap(),
                "{case}"
            );
        }
    }
}
