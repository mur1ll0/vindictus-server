use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    pet::{self as wire, Kind, Message, PetStatusInfo},
    server_profile::REGISTERED_CLASSES,
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
        "move_pet_slot" => Kind::MovePetSlot,
        "remove_pet" => Kind::RemovePet,
        "select_pet" => Kind::SelectPet,
        "query_pet_list" => Kind::QueryPetList,
        "pet_revived" => Kind::PetRevived,
        "pet_killed" => Kind::PetKilled,
        "remove_pet_skill" => Kind::RemovePetSkill,
        "pet_create_name_check" => Kind::PetCreateNameCheck,
        "pet_create_name_check_result" => Kind::PetCreateNameCheckResult,
        "pet_create" => Kind::PetCreate,
        "update_current_pet" => Kind::UpdateCurrentPet,
        "pet_list" => Kind::PetList,
        "pet_feed_list" => Kind::PetFeedList,
        "request_pet_food_share" => Kind::RequestPetFoodShare,
        "request_pet_food_unshare" => Kind::RequestPetFoodUnshare,
        "pet_change_name_check" => Kind::PetChangeNameCheck,
        "pet_change_name_check_result" => Kind::PetChangeNameCheckResult,
        "pet_change_name" => Kind::PetChangeName,
        "pet_change_name_result" => Kind::PetChangeNameResult,
        "pet_operation" => Kind::PetOperation,
        _ => panic!("unknown fixture type"),
    }
}
fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
/// PetStatusInfo against its captured properties.
fn pet(ours: Option<&PetStatusInfo>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(p) = ours else { return };
    assert_eq!(p.pet_id, int(&theirs["PetID"]), "case {n}");
    assert_eq!(
        p.pet_name.as_deref(),
        theirs["PetName"].as_str(),
        "case {n}"
    );
    for (ours, key) in [
        (p.pet_type, "PetType"),
        (p.slot, "Slot"),
        (p.level, "Level"),
        (p.exp, "Exp"),
        (p.desire, "Desire"),
        (p.remain_active_time, "RemainActiveTime"),
    ] {
        assert_eq!(i64::from(ours), int(&theirs[key]), "case {n} {key}");
    }
    assert_eq!(
        i64::from(p.pet_status),
        int(&theirs["PetStatus"]),
        "case {n}"
    );
    let date = &theirs["RemainExpiredTime"];
    assert_eq!(
        p.remain_expired_time.ticks(),
        int(&date["ticks"]),
        "case {n}"
    );
    assert_eq!(
        (p.remain_expired_time.0 >> 62) as i64,
        int(&date["kind"]),
        "case {n}"
    );
    let stat = &theirs["Stat"];
    assert_eq!(p.stat.is_none(), stat.is_null(), "case {n}");
    if let Some(s) = &p.stat {
        for (ours, key) in [
            (s.required_exp, "RequiredExp"),
            (s.max_exp, "MaxExp"),
            (s.hp, "Hp"),
            (s.res_damage, "ResDamage"),
            (s.hp_recovery, "HpRecovery"),
            (s.def_break, "DefBreak"),
            (s.atk_balance, "AtkBalance"),
            (s.atk, "Atk"),
            (s.def, "Def"),
            (s.critical, "Critical"),
            (s.res_critical, "ResCritical"),
        ] {
            assert_eq!(i64::from(ours), int(&stat[key]), "case {n} {key}");
        }
    }
    let skills = &theirs["Skills"];
    assert_eq!(p.skills.is_none(), skills.is_null(), "case {n}");
    if let Some(list) = &p.skills {
        let rows = skills.as_array().unwrap();
        assert_eq!(list.len(), rows.len(), "case {n}");
        for (s, row) in list.iter().zip(rows) {
            let s = s.as_ref().unwrap();
            assert_eq!(i64::from(s.skill_id), int(&row["SkillID"]), "case {n}");
            assert_eq!(i64::from(s.slot_order), int(&row["SlotOrder"]), "case {n}");
            assert_eq!(i64::from(s.open_level), int(&row["OpenLevel"]), "case {n}");
            assert_eq!(
                s.has_expire_date_time_info,
                row["HasExpireDateTimeInfo"].as_bool().unwrap(),
                "case {n}"
            );
            assert_eq!(
                s.expire_date_time_diff,
                int(&row["ExpireDateTimeDiff"]),
                "case {n}"
            );
        }
    }
    let accessories = &theirs["Accessories"];
    assert_eq!(p.accessories.is_none(), accessories.is_null(), "case {n}");
    if let Some(list) = &p.accessories {
        let rows = accessories.as_array().unwrap();
        assert_eq!(list.len(), rows.len(), "case {n}");
        for (a, row) in list.iter().zip(rows) {
            let a = a.as_ref().unwrap();
            assert_eq!(
                a.item_class.as_deref(),
                row["ItemClass"].as_str(),
                "case {n}"
            );
            assert_eq!(i64::from(a.slot_order), int(&row["SlotOrder"]), "case {n}");
            assert_eq!(
                i64::from(a.accessory_size),
                int(&row["AccessorySize"]),
                "case {n}"
            );
            assert_eq!(
                i64::from(a.remaining_time),
                int(&row["RemainingTime"]),
                "case {n}"
            );
        }
    }
}
fn check(case: &Value, n: usize) {
    let kind = kind(case["kind"].as_str().unwrap());
    let category = case["category"].as_i64().unwrap() as i32;
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(
        decoded.is_none(),
        case["is_null"].as_bool().unwrap(),
        "case {n}"
    );
    assert_eq!(
        wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
        decoded,
        "case {n}"
    );
    assert_eq!(
        wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY)
            .unwrap()
            .as_slice(),
        typed.as_slice(),
        "case {n}"
    );
    for cut in 1..typed.len() {
        assert!(
            wire::decode(kind, &typed[..cut], DEFAULT_MAX_BODY).is_err(),
            "case {n} cut {cut}"
        );
    }
    let f = &case["fields"];
    match decoded {
        None | Some(Message::QueryPetList) => {}
        Some(Message::PetList(m)) => {
            assert_eq!(
                m.is_total,
                f["IsTotalPetList"].as_bool().unwrap(),
                "case {n}"
            );
            let rows = &f["PetList"];
            assert_eq!(m.pets.is_none(), rows.is_null(), "case {n}");
            if let Some(list) = &m.pets {
                let rows = rows.as_array().unwrap();
                assert_eq!(list.len(), rows.len(), "case {n}");
                for (p, row) in list.iter().zip(rows) {
                    pet(p.as_ref(), row, n);
                }
            }
        }
        Some(Message::UpdateCurrentPet(m)) => {
            assert_eq!(i64::from(m.uid), int(&f["UID"]), "case {n}");
            pet(m.current_pet.as_ref(), &f["CurrentPet"], n);
        }
        Some(Message::PetFeedList(m)) => {
            assert_eq!(
                m.is_total,
                f["IsTotalPetList"].as_bool().unwrap(),
                "case {n}"
            );
            let rows = &f["PetFeedList"];
            assert_eq!(m.feeds.is_none(), rows.is_null(), "case {n}");
            if let Some(list) = &m.feeds {
                let rows = rows.as_array().unwrap();
                assert_eq!(list.len(), rows.len(), "case {n}");
                for (e, row) in list.iter().zip(rows) {
                    assert_eq!(e.is_none(), row.is_null(), "case {n}");
                    if let Some(e) = e {
                        assert_eq!(i64::from(e.feed_type), int(&row["Type"]), "case {n}");
                        assert_eq!(i64::from(e.count), int(&row["Count"]), "case {n}");
                    }
                }
            }
        }
        Some(Message::SelectPet(id)) | Some(Message::RemovePet(id)) => {
            assert_eq!(id, int(&f["PetID"]), "case {n}")
        }
        Some(Message::RequestPetFoodShare(id)) => assert_eq!(id, int(&f["ItemID"]), "case {n}"),
        Some(Message::RequestPetFoodUnshare(class)) => {
            assert_eq!(class.as_deref(), f["ItemClass"].as_str(), "case {n}")
        }
        Some(Message::MovePetSlot(m)) => {
            assert_eq!(
                (m.pet_id, i64::from(m.dest_slot_no)),
                (int(&f["PetID"]), int(&f["DestSlotNo"])),
                "case {n}"
            )
        }
        Some(Message::PetKilled(m)) => {
            assert_eq!(
                (i64::from(m.tag), m.pet_id),
                (int(&f["Tag"]), int(&f["PetID"])),
                "case {n}"
            )
        }
        Some(Message::PetRevived(m)) => {
            assert_eq!(i64::from(m.caster_tag), int(&f["CasterTag"]), "case {n}");
            assert_eq!(i64::from(m.reviver_tag), int(&f["ReviverTag"]), "case {n}");
            assert_eq!(m.pet_id, int(&f["PetID"]), "case {n}");
            assert_eq!(m.method.as_deref(), f["Method"].as_str(), "case {n}");
        }
        Some(Message::RemovePetSkill(m)) => {
            assert_eq!(
                (m.pet_id, i64::from(m.pet_skill_id)),
                (int(&f["PetID"]), int(&f["PetSkillID"])),
                "case {n}"
            )
        }
        Some(Message::PetCreateNameCheck(m)) | Some(Message::PetCreate(m)) => {
            assert_eq!(m.item_id, int(&f["ItemID"]), "case {n}");
            assert_eq!(m.pet_name.as_deref(), f["PetName"].as_str(), "case {n}");
        }
        Some(Message::PetCreateNameCheckResult(m)) => {
            assert_eq!(m.item_id, int(&f["ItemID"]), "case {n}");
            assert_eq!(m.pet_name.as_deref(), f["PetName"].as_str(), "case {n}");
            assert_eq!(m.is_success, f["IsSuccess"].as_bool().unwrap(), "case {n}");
        }
        Some(Message::PetChangeNameCheck(m)) | Some(Message::PetChangeName(m)) => {
            assert_eq!(
                (m.item_id, m.pet_id),
                (int(&f["ItemID"]), int(&f["PetID"])),
                "case {n}"
            );
            assert_eq!(m.pet_name.as_deref(), f["PetName"].as_str(), "case {n}");
        }
        Some(Message::PetChangeNameCheckResult(m)) => {
            assert_eq!(m.is_success, f["IsSuccess"].as_bool().unwrap(), "case {n}");
            assert_eq!(
                (m.item_id, m.pet_id),
                (int(&f["ItemID"]), int(&f["PetID"])),
                "case {n}"
            );
            assert_eq!(m.pet_name.as_deref(), f["PetName"].as_str(), "case {n}");
            assert_eq!(
                m.result_type.as_deref(),
                f["ResultType"].as_str(),
                "case {n}"
            );
        }
        Some(Message::PetChangeNameResult(m)) => {
            assert_eq!(m.is_success, f["IsSuccess"].as_bool().unwrap(), "case {n}");
            assert_eq!(m.pet_name.as_deref(), f["PetName"].as_str(), "case {n}");
            assert_eq!(
                m.result_type.as_deref(),
                f["ResultType"].as_str(),
                "case {n}"
            );
        }
        Some(Message::PetOperation(m)) => {
            assert_eq!(
                i64::from(m.operation_code),
                int(&f["OperationCode"]),
                "case {n}"
            );
            assert_eq!(m.pet_id, int(&f["PetID"]), "case {n}");
            assert_eq!(m.arg.as_deref(), f["Arg"].as_str(), "case {n}");
            for (ours, key) in [
                (m.value1, "Value1"),
                (m.value2, "Value2"),
                (m.value3, "Value3"),
            ] {
                assert_eq!(i64::from(ours), int(&f[key]), "case {n}");
            }
        }
    }
}

#[test]
fn framework_pet_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-pet-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    for kind in Kind::INGRESS.iter().chain(&[
        Kind::PetCreateNameCheckResult,
        Kind::UpdateCurrentPet,
        Kind::PetList,
        Kind::PetFeedList,
        Kind::PetChangeNameCheckResult,
        Kind::PetChangeNameResult,
    ]) {
        assert!(
            REGISTERED_CLASSES
                .iter()
                .any(|r| r.category_id == kind.category() && r.type_id == kind.type_id())
        );
    }
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 195);
    for (n, case) in cases.iter().enumerate() {
        check(case, n);
    }
}

#[test]
fn utc_date_time_keeps_the_kind_bits() {
    let date = wire::DateTimeData::utc(634_647_492_000_000_000);
    assert_eq!(date.0, 0x48ce_b888_0e40_a800);
    assert_eq!(date.ticks(), 634_647_492_000_000_000);
}
