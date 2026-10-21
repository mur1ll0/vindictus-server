use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY, character_list,
    inventory::{self as wire, Kind, Message},
    server_profile::REGISTERED_CLASSES,
};
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn registered(category: i32, type_id: uuid::Uuid) {
    assert!(
        REGISTERED_CLASSES
            .iter()
            .any(|r| r.category_id == category && r.type_id == type_id)
    );
}
/// Unregistered bodies carry the type GUID; registered bodies use the category.
fn fields(category: i32, expected: i32, type_id: uuid::Uuid, body: &[u8]) -> &[u8] {
    if category == expected {
        body
    } else {
        assert_eq!(category, 0);
        // A null object is written with the System.Object GUID and a null marker.
        let null = uuid::Uuid::from_u128(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21);
        if &body[..16] == null.as_bytes() {
            assert_eq!(&body[16..], [0]);
            return &body[16..];
        }
        assert_eq!(&body[..16], type_id.as_bytes());
        &body[16..]
    }
}
fn check_inventory(kind: Kind, case: &Value, n: usize) {
    let category = case["category"].as_i64().unwrap() as i32;
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let null = case["is_null"].as_bool().unwrap();
    let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(decoded.is_none(), null, "case {n}");
    assert!(
        wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap() == decoded,
        "case {n}"
    );
    assert_eq!(
        wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY)
            .unwrap()
            .as_slice(),
        typed.as_slice(),
        "case {n}"
    );
    let f = &case["fields"];
    match decoded {
        None => {}
        Some(Message::ItemFail(message)) => {
            let source = &f["Message"];
            assert_eq!(message.is_none(), source.is_null(), "case {n}");
            if let Some(m) = message {
                assert_eq!(m.format.as_deref(), source["Format"].as_str(), "case {n}");
                assert_eq!(
                    m.parameters.map(|p| p.len()),
                    source["Parameters"].as_array().map(Vec::len),
                    "case {n}"
                );
            }
        }
        Some(Message::UpdateInventoryInfo(slots)) => {
            let source = &f["slotInfos"];
            assert_eq!(slots.is_none(), source.is_null(), "case {n}");
            if let Some(slots) = slots {
                let rows = source.as_array().unwrap();
                assert_eq!(slots.len(), rows.len(), "case {n}");
                for (slot, row) in slots.iter().zip(rows) {
                    assert_eq!(
                        slot.as_ref().map(|s| s.id),
                        row["ItemID"].as_i64(),
                        "case {n}"
                    );
                }
            }
        }
        Some(Message::MoveInventoryItem(m)) => {
            assert_eq!(m.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(m.storage),
                f["Storage"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(i64::from(m.slot), f["Target"].as_i64().unwrap(), "case {n}");
        }
        Some(Message::EquipBundle(b)) => {
            assert_eq!(b.class.as_deref(), f["ItemClass"].as_str(), "case {n}");
            assert_eq!(
                i64::from(b.quick_slot),
                f["QuickSlotID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::QuickSlotInfo(info)) => {
            assert_eq!(info.is_none(), f["info"].is_null(), "case {n}");
        }
        Some(Message::QueryAp) => {}
        Some(Message::Ap(a)) => {
            assert_eq!(
                a.next_bonus_ms,
                f["NextBonusTimeTicks"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(i64::from(a.ap), f["AP"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                i64::from(a.max_ap),
                f["MaxAP"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                i64::from(a.bonus_interval),
                f["APBonusInterval"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::UseInventoryItem(u)) => {
            assert_eq!(u.item_id, f["ItemID"].as_i64().unwrap(), "case {n}");
            assert_eq!(
                u.target_item_id,
                f["TargetItemID"].as_i64().unwrap(),
                "case {n}"
            );
        }
        Some(Message::UseInventoryItemOk(class)) => {
            assert_eq!(class.as_deref(), f["UseItemClass"].as_str(), "case {n}");
        }
        Some(Message::QuerySkillList) => {}
        Some(Message::SkillList(l)) => {
            assert_eq!(
                l.learning.as_deref(),
                f["LearningSkillID"].as_str(),
                "case {n}"
            );
            assert_eq!(
                i64::from(l.current_ap),
                f["CurrentAp"].as_i64().unwrap(),
                "case {n}"
            );
            assert_eq!(
                l.reset_vocation,
                f["IsResetVocation"].as_bool().unwrap(),
                "case {n}"
            );
            let rows = f["SkillList"].as_array();
            assert_eq!(
                l.skills.as_ref().map(Vec::len),
                rows.map(Vec::len),
                "case {n}"
            );
            if let (Some(skills), Some(rows)) = (l.skills, rows) {
                for (skill, row) in skills.iter().zip(rows) {
                    assert_eq!(skill.is_none(), row.is_null(), "case {n}");
                    if let Some(b) = skill {
                        assert_eq!(b.id.as_deref(), row["SkillID"].as_str(), "case {n}");
                        assert_eq!(
                            i64::from(b.untrain_ap),
                            row["UntrainSkillAP"].as_i64().unwrap(),
                            "case {n}"
                        );
                        assert_eq!(
                            b.enhances.as_ref().map(Vec::len),
                            row["Enhances"].as_array().map(Vec::len),
                            "case {n}"
                        );
                    }
                }
            }
        }
        Some(_) => panic!("unexpected kind"),
    }
    for cut in 0..typed.len() {
        assert!(wire::decode(kind, &typed[..cut], DEFAULT_MAX_BODY).is_err());
    }
    let mut extra = typed.clone();
    extra.push(0);
    assert!(wire::decode(kind, &extra, DEFAULT_MAX_BODY).is_err());
}
fn check_costume(case: &Value, n: usize) {
    let category = case["category"].as_i64().unwrap() as i32;
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let null = case["is_null"].as_bool().unwrap();
    let decoded = character_list::decode_costume_update(&typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(decoded.is_none(), null, "case {n}");
    let ingress = fields(
        category,
        character_list::COSTUME_UPDATE_CATEGORY,
        character_list::COSTUME_UPDATE_TYPE_ID,
        &body,
    );
    assert!(
        character_list::decode_costume_update(ingress, DEFAULT_MAX_BODY).unwrap() == decoded,
        "case {n}"
    );
    assert_eq!(
        character_list::encode_costume_update(
            decoded.as_ref().map(Option::as_ref),
            DEFAULT_MAX_BODY
        )
        .unwrap()
        .as_slice(),
        typed.as_slice(),
        "case {n}"
    );
    if let Some(info) = decoded {
        let source = &case["fields"]["CostumeInfo"];
        assert_eq!(info.is_none(), source.is_null(), "case {n}");
        if let Some(c) = info {
            assert_eq!(
                i64::from(c.shineness),
                source["Shineness"].as_i64().unwrap()
            );
            assert_eq!(
                i64::from(c.town_effect),
                source["TownEffect"].as_i64().unwrap()
            );
            assert_eq!(
                c.costume_type_info.map(|d| d.len()),
                source["CostumeTypeInfo"].as_array().map(Vec::len)
            );
        }
    }
    for cut in 0..typed.len() {
        assert!(character_list::decode_costume_update(&typed[..cut], DEFAULT_MAX_BODY).is_err());
    }
}
#[test]
fn framework_item_sync_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-item-sync-wire.json")).unwrap();
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    registered(
        Kind::UpdateInventoryInfo.category(),
        Kind::UpdateInventoryInfo.type_id(),
    );
    registered(Kind::ItemFail.category(), Kind::ItemFail.type_id());
    registered(Kind::EquipBundle.category(), Kind::EquipBundle.type_id());
    registered(
        Kind::QuickSlotInfo.category(),
        Kind::QuickSlotInfo.type_id(),
    );
    registered(
        Kind::MoveInventoryItem.category(),
        Kind::MoveInventoryItem.type_id(),
    );
    registered(
        character_list::COSTUME_UPDATE_CATEGORY,
        character_list::COSTUME_UPDATE_TYPE_ID,
    );
    for kind in [
        Kind::QueryAp,
        Kind::Ap,
        Kind::UseInventoryItem,
        Kind::UseInventoryItemOk,
        Kind::QuerySkillList,
        Kind::SkillList,
    ] {
        registered(kind.category(), kind.type_id());
    }
    for profile in profiles {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 618);
        for (n, case) in cases.iter().enumerate() {
            match case["kind"].as_str().unwrap() {
                "update_inventory_info" => check_inventory(Kind::UpdateInventoryInfo, case, n),
                "item_fail" => check_inventory(Kind::ItemFail, case, n),
                "move_inventory_item" => check_inventory(Kind::MoveInventoryItem, case, n),
                "equip_bundle" => check_inventory(Kind::EquipBundle, case, n),
                "quick_slot_info" => check_inventory(Kind::QuickSlotInfo, case, n),
                "query_ap" => check_inventory(Kind::QueryAp, case, n),
                "ap" => check_inventory(Kind::Ap, case, n),
                "use_inventory_item" => check_inventory(Kind::UseInventoryItem, case, n),
                "use_inventory_item_ok" => check_inventory(Kind::UseInventoryItemOk, case, n),
                "query_skill_list" => check_inventory(Kind::QuerySkillList, case, n),
                "skill_list" => check_inventory(Kind::SkillList, case, n),
                "costume_update" => check_costume(case, n),
                _ => panic!("unknown fixture type"),
            }
        }
    }
}
#[test]
fn item_fail_message_matches_source_dialog_shape() {
    let message = Message::ItemFail(Some(wire::HeroesString {
        format: Some("GameUI_Heroes_EquipItemFail_System".to_owned()),
        parameters: Some(Vec::new()),
    }));
    let body = wire::encode(Kind::ItemFail, Some(&message), DEFAULT_MAX_BODY).unwrap();
    assert!(wire::decode(Kind::ItemFail, &body, DEFAULT_MAX_BODY).unwrap() == Some(message));
}
