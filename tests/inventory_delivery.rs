use serde_json::{Value, json};
use vindictus_server::{
    application::{
        character_creation::EntityAuthority,
        inventory_delivery::{self as delivery, DeliveryClock, Inputs, Journals},
    },
    domain::{
        initial_inventory::{InitialRequest, InventoryError},
        item_colors::{ColorRandom, Materials},
        item_stats::ItemStat,
        legacy_user_data::SqlDateTime2,
    },
};
fn utf(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[test]
fn delivery_factory_events_and_preallocation_requests_match_framework() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-inventory-delivery.json")).unwrap();
    let cases = f["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 32);
    for c in cases {
        let stat = ItemStat::from_raw(&[], &[], [50, 0, 0, 0]).unwrap();
        let raw = c["input"].as_array().unwrap();
        let keys = raw
            .iter()
            .map(|r| utf(r[0].as_str().unwrap()))
            .collect::<Vec<_>>();
        let requests = raw
            .iter()
            .enumerate()
            .map(|(i, r)| InitialRequest {
                class: &keys[i],
                count: r[1].as_i64().unwrap() as i32,
                max_stack: r[2].as_i64().unwrap() as i32,
                unique: 0,
                storage: r[4].as_u64().unwrap() as u8,
                expire_in: None,
                equipment: r[3].as_bool().unwrap(),
                base: &stat,
                materials: [None; 3],
                gold: false,
            })
            .collect::<Vec<_>>();
        let mut authority = EntityAuthority::new(17);
        let mut tick = 0;
        let mut civil = || {
            let n = 637999999999999999 + tick * 17001;
            tick += 1;
            Ok(SqlDateTime2::from_ticks(n).unwrap())
        };
        let mut p = delivery::prepare(
            Inputs {
                requests: &requests,
                materials: &Materials::default(),
                maximum_durability: 100,
                bundle_exceed: 0,
            },
            1,
            Journals {
                operation: 17,
                ledger: 31,
                auto_logging: true,
                shared_inventory: true,
            },
            &mut ColorRandom::from_seed(1),
            &mut DeliveryClock {
                authority: &mut authority,
                utc: &mut || Ok(SqlDateTime2::from_ticks(638448480000000000).unwrap()),
                civil: &mut civil,
            },
        )
        .unwrap();
        let item_ids = p
            .inventory()
            .items()
            .iter()
            .map(|r| r.id())
            .collect::<Vec<_>>();
        assert_eq!(authority.counter(), *item_ids.last().unwrap());
        let events = p
            .events()
            .iter()
            .map(|r| {
                json!([
                    r.data.item_id,
                    String::from_utf16(
                        &r.data
                            .class()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u16::from_le_bytes(*b))
                            .collect::<Vec<_>>()
                    )
                    .unwrap(),
                    r.data.count,
                    r.detail,
                    r.before_id,
                    r.data.system,
                    r.data.colors,
                    r.data.reduced,
                    r.data.bonus,
                    r.captured.ticks()
                ])
            })
            .collect::<Vec<_>>();
        let mut original = c["events"].as_array().unwrap().clone();
        assert_eq!(events.len(), item_ids.len());
        for (i, v) in original.iter_mut().enumerate() {
            v[0] = json!(item_ids[i]);
        }
        assert_eq!(json!(events), json!(original));
        // Original allocator is instrumented here; IDs must still be the same IDs used by storage.
        for (i, e) in p.events().iter().enumerate() {
            assert_eq!(e.data.item_id, item_ids[i]);
        }
        let last = p.events().last().unwrap();
        let class_index = keys
            .iter()
            .position(|k| k.as_slice() == last.data.class())
            .unwrap();
        assert_eq!(json!(last.after.shared), c["shared_counts"][class_index]);
        let rows = p
            .ledgers()
            .iter()
            .map(|l| {
                json!([
                    l.data.item_id,
                    String::from_utf16(
                        &l.data
                            .class()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u16::from_le_bytes(*b))
                            .collect::<Vec<_>>()
                    )
                    .unwrap(),
                    l.data.count,
                    l.data.colors,
                    l.data.reduced,
                    l.data.bonus,
                    l.bundle
                ])
            })
            .collect::<Vec<_>>();
        assert_eq!(json!(rows), c["ledger"]);
        let expected = c["counts"].as_array().unwrap();
        for (i, l) in p.ledgers().iter().enumerate() {
            assert_eq!(json!(l.bundle), expected[i]);
        }
    }
}
#[test]
fn delivery_disabled_logging_does_not_sample_process_clock_and_late_failure_consumes_authority() {
    let stat = ItemStat::from_raw(&[], &[], [50, 0, 0, 0]).unwrap();
    let k = utf("synthetic");
    let r = InitialRequest {
        class: &k,
        count: 1,
        max_stack: 1,
        unique: 0,
        storage: 1,
        expire_in: None,
        equipment: true,
        base: &stat,
        materials: [None; 3],
        gold: false,
    };
    let materials = Materials::default();
    let inputs = || Inputs {
        requests: std::slice::from_ref(&r),
        materials: &materials,
        maximum_durability: 100,
        bundle_exceed: 0,
    };
    let mut authority = EntityAuthority::new(17);
    let mut random = ColorRandom::from_seed(1);
    let mut p = delivery::prepare(
        inputs(),
        1,
        Journals {
            operation: -1,
            ledger: 31,
            auto_logging: false,
            shared_inventory: false,
        },
        &mut random,
        &mut DeliveryClock {
            authority: &mut authority,
            utc: &mut || Ok(SqlDateTime2::from_ticks(638448480000000000).unwrap()),
            civil: &mut || panic!("disabled clock"),
        },
    )
    .unwrap();
    assert!(p.events().is_empty());
    assert_eq!(p.ledgers().len(), 1);
    let before = authority.counter();
    let e = delivery::prepare(
        inputs(),
        1,
        Journals {
            operation: 17,
            ledger: 31,
            auto_logging: true,
            shared_inventory: false,
        },
        &mut random,
        &mut DeliveryClock {
            authority: &mut authority,
            utc: &mut || Ok(SqlDateTime2::from_ticks(638448480000000000).unwrap()),
            civil: &mut || Err(InventoryError::Clock),
        },
    );
    assert!(matches!(e, Err(InventoryError::Clock)));
    assert!(authority.counter() > before);
}

#[test]
fn shared_snapshot_input_rejects_missing_fields_bad_bytes_duplicate_keys_and_bounds() {
    use vindictus_server::infrastructure::shared_item_state::read_snapshot;
    let valid = json!({"format_version":1,"source":"study_heroes.shared-items","last_value":null,"items":[],"storage":[]});
    assert!(read_snapshot(serde_json::to_vec(&valid).unwrap().as_slice()).is_ok());
    let mut v = valid.clone();
    v.as_object_mut().unwrap().remove("last_value");
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    let mut v = valid.clone();
    v["items"] = json!([{"id":"1","owner_uid":null,"collection":256,"slot":null}]);
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    let mut duplicated = valid.clone();
    let item = json!({"id":"1","owner_uid":null,"collection":null,"slot":null});
    duplicated["items"] = json!([item.clone(), item]);
    assert!(read_snapshot(serde_json::to_vec(&duplicated).unwrap().as_slice()).is_err());
    let mut v = valid;
    v["last_value"] = json!("1");
    v["storage"] =
        json!([{"id":"1","owner_uid":1,"storage_type":0,"storage":20,"expire_binary":"00"}]);
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
}
