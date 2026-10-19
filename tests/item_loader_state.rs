use vindictus_server::infrastructure::item_loader_state::read_snapshot;
mod fixture {
    include!("fixtures/item_loader_support.rs");
}
#[test]
fn item_loader_parser_preserves_raw_utf16_signed_fields_and_datetime2_extremes() {
    for v in [fixture::snapshot(), fixture::empty()] {
        assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_ok())
    }
}
#[test]
fn item_loader_parser_refuses_loss_ambiguity_and_identity_mismatch() {
    use serde_json::json;
    for mode in 0..12 {
        let mut v = fixture::snapshot();
        match mode {
            0 => v["quick_slots"][0]["item_class_utf16"] = json!("410"),
            1 => {
                let r = v["quick_slots"][0].clone();
                v["quick_slots"].as_array_mut().unwrap().push(r)
            }
            2 => v["source_last"][0] = json!("10"),
            3 => v["source_last"][1] = json!("2147483647"),
            4 => v["new_recipes"][0]["new_until_ticks"] = json!("3155378976000000000"),
            5 => v["recipes"][0]["acquired_ticks"] = json!(null),
            6 => v["schema_sha256"] = json!("a".repeat(64)),
            7 => v["quick_slots"][0]["owner_id"] = json!(1),
            8 => {
                v.as_object_mut().unwrap().remove("expanded_slots");
            }
            9 => v["manufacture"][0]["manufacture_utf16"] = json!("4100".repeat(17)),
            10 => v["recipes"][0]["recipe_utf16"] = json!("4100".repeat(129)),
            _ => v["new_recipes"][0]["recipe_utf16"] = json!("4100".repeat(101)),
        };
        assert!(
            read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err(),
            "case {mode}"
        )
    }
    assert!(read_snapshot(vec![b' '; 16 * 1024 * 1024 + 1].as_slice()).is_err());
}
