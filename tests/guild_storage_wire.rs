use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    guild_storage::{self as wire, Kind, Message},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

fn kind(name: &str) -> Kind {
    match name {
        "guild_inventory_info" => Kind::GuildInventoryInfo,
        "add_guild_storage_item" => Kind::AddGuildStorageItem,
        "pick_guild_storage_item" => Kind::PickGuildStorageItem,
        "purchase_guild_storage" => Kind::PurchaseGuildStorage,
        "arrange_guild_storage_item" => Kind::ArrangeGuildStorageItem,
        "handle_guild_storage_session" => Kind::HandleGuildStorageSession,
        "update_guild_storage_setting" => Kind::UpdateGuildStorageSetting,
        "guild_storage_logs" => Kind::GuildStorageLogs,
        _ => panic!("unknown fixture type"),
    }
}

/// System.Object's GUID: the envelope of a null unregistered message.
const NULL_OBJECT: u128 = 0x81c5fe01_027c_3e1c_98d5_da9c9862aa21;

#[test]
fn framework_guild_storage_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-guild-storage-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 34);
    let null_object = uuid::Uuid::from_u128(NULL_OBJECT);
    for (n, case) in cases.iter().enumerate() {
        let kind = kind(case["kind"].as_str().unwrap());
        let body = bytes(&case["body"]);
        let typed = bytes(&case["typed_body"]);
        let category = case["category"].as_i64().unwrap() as i32;
        if kind.category() == 0 {
            assert_eq!(category, 0, "case {n}");
            if typed[..16] == *null_object.as_bytes() {
                assert_eq!(
                    (&typed[16..], case["is_null"].as_bool()),
                    (&[0u8][..], Some(true))
                );
                continue;
            }
            assert_eq!(&typed[..16], kind.type_id().as_bytes(), "case {n}");
        } else if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let payload = if kind.category() == 0 {
            &typed[16..]
        } else {
            &typed[..]
        };
        let decoded = wire::decode(kind, payload, DEFAULT_MAX_BODY)
            .unwrap_or_else(|e| panic!("case {n} {:?}: {e:?}", case["kind"]));
        assert_eq!(decoded.is_none(), case["fields"].is_null(), "case {n}");
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            payload,
            "case {n}"
        );
        if let Some(m) = &decoded {
            // The sent packet is the typed form: GUID envelope when unregistered.
            assert_eq!(
                wire::packet(m, DEFAULT_MAX_BODY).unwrap(),
                (kind.category(), typed.clone()),
                "case {n}"
            );
        }
        if Kind::INGRESS.contains(&kind) {
            assert_eq!(wire::ingress_kind(category, &body), Some(kind), "case {n}");
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "ingress case {n}"
            );
        }
    }
    // Field mapping of the nested log rows.
    let case = cases
        .iter()
        .find(|c| c["kind"] == "guild_storage_logs" && !c["fields"].is_null())
        .unwrap();
    let typed = bytes(&case["typed_body"]);
    let Some(Message::GuildStorageLogs(logs)) =
        wire::decode(Kind::GuildStorageLogs, &typed[16..], DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("no GuildStorageLogsMessage")
    };
    let theirs = case["fields"]["BriefLogs"].as_array().unwrap();
    for (ours, t) in logs.brief_logs.unwrap().iter().zip(theirs) {
        let Some(o) = ours else {
            assert!(t.is_null());
            continue;
        };
        assert_eq!(o.character_name.as_deref(), t["CharacterName"].as_str());
        assert_eq!(
            i64::from(o.operation_type),
            t["OperationType"].as_i64().unwrap()
        );
        assert_eq!(
            (i64::from(o.add_count), i64::from(o.pick_count)),
            (
                t["AddCount"].as_i64().unwrap(),
                t["PickCount"].as_i64().unwrap()
            )
        );
        assert_eq!(
            (i64::from(o.datestamp), i64::from(o.timestamp)),
            (
                t["Datestamp"].as_i64().unwrap(),
                t["Timestamp"].as_i64().unwrap()
            )
        );
    }
    let theirs = case["fields"]["ItemLogs"].as_array().unwrap();
    for (ours, t) in logs.item_logs.unwrap().iter().zip(theirs) {
        let o = ours.as_ref().unwrap();
        assert_eq!(o.character_name.as_deref(), t["CharacterName"].as_str());
        assert_eq!(o.is_add_item, t["IsAddItem"].as_bool().unwrap());
        assert_eq!(o.item_class.as_deref(), t["ItemClass"].as_str());
        assert_eq!(i64::from(o.count), t["Count"].as_i64().unwrap());
        assert_eq!(
            [o.color1, o.color2, o.color3].map(i64::from),
            ["Color1", "Color2", "Color3"].map(|c| t[c].as_i64().unwrap())
        );
    }
}

#[test]
fn guild_storage_ingress_needs_the_guid_envelope() {
    let session =
        wire::packet(&Message::HandleGuildStorageSession(true), DEFAULT_MAX_BODY).unwrap();
    assert_eq!(session.0, 0);
    assert_eq!(
        wire::ingress_kind(0, &session.1),
        Some(Kind::HandleGuildStorageSession)
    );
    // The bare body, or any category, is not a guild storage message.
    assert_eq!(wire::ingress_kind(0, &session.1[16..]), None);
    assert_eq!(wire::ingress_kind(87, &session.1), None);
}
