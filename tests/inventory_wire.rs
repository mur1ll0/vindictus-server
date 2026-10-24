use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    inventory::{self as wire, *},
};
fn opt<T>(v: &Value, f: impl FnOnce(&Value) -> T) -> Option<T> {
    if v.is_null() { None } else { Some(f(v)) }
}
fn list<T>(v: &Value, f: impl Fn(&Value) -> T) -> Option<Vec<T>> {
    opt(v, |v| v.as_array().unwrap().iter().map(f).collect())
}
fn string(v: &Value) -> Option<String> {
    opt(v, |v| v.as_str().unwrap().to_owned())
}
fn i(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn l(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn dict<T>(v: &Value, f: impl Fn(&Value) -> T) -> Dict<T> {
    Dict(list(v, |r| (i(&r["key"]), f(&r["value"]))))
}
fn storage(v: &Value) -> Storage {
    Storage {
        id: v["StorageID"].as_u64().unwrap() as u8,
        available: v["IsAvailable"].as_u64().unwrap() as u8,
        last_seconds: i(&v["LastSecsToExpire"]),
        name: string(&v["StorageName"]),
        tag: i(&v["StorageTag"]),
    }
}
fn attr(v: &Value) -> Attribute {
    Attribute {
        name: string(&v["AttributeName"]),
        value: string(&v["Value"]),
        arg: i(&v["Arg"]),
        arg2: i(&v["Arg2"]),
    }
}
fn slot(v: &Value) -> Slot {
    Slot {
        id: l(&v["ItemID"]),
        class: string(&v["ItemClass"]),
        count: i(&v["Num"]),
        tab: i(&v["Tab"]),
        slot: i(&v["Slot"]),
        expireable: v["IsExpireable"].as_bool().unwrap(),
        expire_diff: l(&v["ExpireDateTimeDiff"]),
        expire_ticks: l(&v["ExpireDateTimeTick"]),
        colors: list(&v["Colors"], i),
        durability: i(&v["Durability"]),
        maximum: i(&v["MaxDurability"]),
        bonus: i(&v["MaxDurabilityBonus"]),
        tradable: v["Tradable"].as_bool().unwrap(),
        expander: string(&v["ExpanderName"]),
        attributes: list(&v["Attributes"], |v| opt(v, attr)),
        prefix: dict(&v["PrefixEnchantStatus"], |v| v.as_bool().unwrap()),
        suffix: dict(&v["SuffixEnchantStatus"], |v| v.as_bool().unwrap()),
        min_price: i(&v["MinPrice"]),
        max_price: i(&v["MaxPrice"]),
        avg_price: i(&v["AvgPrice"]),
        min_up_diff: l(&v["MinUpTimeDateTimeDiff"]),
    }
}
fn inventory(v: &Value) -> Inventory {
    Inventory {
        storages: list(&v["StorageInfos"], |v| opt(v, storage)),
        slots: list(&v["SlotInfos"], |v| opt(v, slot)),
        equipment: dict(&v["EquipmentInfo"], l),
        quick_slots: opt(&v["QuickSlotInfo"], |v| QuickSlots {
            classes: dict(&v["SlotItemClasses"], string),
        }),
        unequippable: list(&v["UnequippableParts"], i),
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-inventory-wire.json")).unwrap()
}
#[test]
fn framework_inventory_snapshots_match_all_envelopes_and_roundtrip() {
    let j = fixture();
    let profiles = j["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    for p in profiles {
        let cases = p["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 48);
        for (n, c) in cases.iter().enumerate() {
            let kind = if c["kind"] == "query_inventory" {
                Kind::QueryInventory
            } else {
                Kind::InventoryInfo
            };
            let expected = opt(&c["fields"], |v| match kind {
                Kind::QueryInventory => Message::QueryInventory,
                Kind::InventoryInfo => Message::InventoryInfo(inventory(v)),
                Kind::UpdateInventoryInfo
                | Kind::HideCostume
                | Kind::TitleList
                | Kind::EnhanceItem
                | Kind::EnhanceItemResult
                | Kind::QueryRestoreItemList
                | Kind::RestoreItemList
                | Kind::EnchantItem
                | Kind::EnchantItemResult
                | Kind::ManufactureInfo
                | Kind::ManufactureCraft
                | Kind::ManufactureLevelUp
                | Kind::UnequippablePartsInfo
                | Kind::MaxDurabilityRepairItem
                | Kind::UpdateStorageInfo
                | Kind::SkillEnhanceSessionStart
                | Kind::SkillEnhanceSessionEnd
                | Kind::SkillEnhanceUseErg
                | Kind::SkillEnhanceUseErgResult
                | Kind::SkillEnhance
                | Kind::SkillEnhanceResult
                | Kind::SkillEnhanceChanged
                | Kind::SkillEnhanceUseDurability
                | Kind::SkillEnhanceRestoreDurability
                | Kind::ItemFail
                | Kind::MoveInventoryItem
                | Kind::EquipBundle
                | Kind::QuickSlotInfo
                | Kind::QueryAp
                | Kind::Ap
                | Kind::UseInventoryItem
                | Kind::UseInventoryItemOk
                | Kind::QuerySkillList
                | Kind::SkillList
                | Kind::LearnSkill
                | Kind::SkillCompletion
                | Kind::QueryCharacterCommonInfo
                | Kind::LearnNewSkill
                | Kind::LearnNewSkillResult
                | Kind::QueryNpcList
                | Kind::NpcList
                | Kind::QueryNpcTalk
                | Kind::NpcTalk
                | Kind::SelectButton
                | Kind::ExecuteNpcServerCommand
                | Kind::SetStoryLineStatus
                | Kind::StoryLines
                | Kind::BuyItem
                | Kind::BuyItemResult
                | Kind::QueryShopItemInfo
                | Kind::SellItem
                | Kind::QueryBuybackList
                | Kind::BuybackListResult
                | Kind::RepairItem
                | Kind::UpdateStoryGuide
                | Kind::StoryVariables
                | Kind::QueryQuestProgress
                | Kind::QuestProgress
                | Kind::UpdateTitle => {
                    unreachable!("covered by item_sync_wire")
                }
            });
            let body: Vec<u8> = c["body"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let typed: Vec<u8> = c["typed_body"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u8)
                .collect();
            let decoded = wire::decode_ingress(
                kind,
                c["category"].as_i64().unwrap() as i32,
                &body,
                DEFAULT_MAX_BODY,
            )
            .unwrap();
            assert!(decoded == expected, "decoded case {n}");
            assert_eq!(
                *wire::encode(kind, expected.as_ref(), DEFAULT_MAX_BODY).unwrap(),
                typed,
                "encoded case {n}"
            );
            for end in 0..typed.len() {
                assert!(
                    wire::decode(kind, &typed[..end], DEFAULT_MAX_BODY).is_err(),
                    "truncation case {n}/{end}"
                )
            }
            let mut trailing = typed.clone();
            trailing.push(0);
            assert!(wire::decode(kind, &trailing, DEFAULT_MAX_BODY).is_err());
            assert!(wire::encode(kind, expected.as_ref(), typed.len() - 1).is_err());
            assert!(wire::decode_ingress(kind, -1, &body, DEFAULT_MAX_BODY).is_err());
        }
    }
}
#[test]
fn inventory_rejects_duplicate_dictionary_keys_and_large_counts() {
    let mut body = vec![1, 0, 0, 3];
    for _ in 0..2 {
        body.extend(1_i32.to_be_bytes());
        body.extend(2_i64.to_be_bytes());
    }
    body.extend([0, 0]);
    assert!(wire::decode(Kind::InventoryInfo, &body, DEFAULT_MAX_BODY).is_err());
    let mut body = vec![1];
    body.extend(vindictus_server::protocol::encode_integer(4098));
    assert!(wire::decode(Kind::InventoryInfo, &body, DEFAULT_MAX_BODY).is_err());
    let inv = Inventory {
        storages: None,
        slots: None,
        equipment: Dict(Some(vec![(1, 2), (1, 3)])),
        quick_slots: None,
        unequippable: None,
    };
    assert!(
        wire::encode(
            Kind::InventoryInfo,
            Some(&Message::InventoryInfo(inv)),
            DEFAULT_MAX_BODY
        )
        .is_err()
    );
    assert!(
        wire::encode(
            Kind::QueryInventory,
            Some(&Message::InventoryInfo(Inventory {
                storages: None,
                slots: None,
                equipment: Dict(None),
                quick_slots: None,
                unequippable: None
            })),
            DEFAULT_MAX_BODY
        )
        .is_err()
    );
}
