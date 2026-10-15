use serde_json::{Value, json};
use vindictus_server::infrastructure::{item_catalog, item_stats::Package};
fn key(s: &str) -> String {
    s.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn fixtures() -> (Value, Value) {
    let a = key("synthetic");
    let b = key("other");
    let c = key("plain");
    let equips = json!([{"item_utf16":a,"feature":"on"},{"item_utf16":a,"feature":"off"},{"item_utf16":a,"feature":"on"},{"item_utf16":b,"feature":null}]);
    let catalog = json!({"format_version":1,"source":"heroesContents.item-catalog-projection","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"region":"STUDY-A","override_region":null,"equip_rows":4,"class_rows":3,"extra_rows":0},"features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"StoryTabInInventory","value":""},{"key":"on","value":"1.1"},{"key":"off","value":null}],"equips":equips,"classes":[{"item_utf16":a,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null},{"item_utf16":b,"feature":null,"max_stack":2,"storage":1,"bind":null,"trade_on_bind":null},{"item_utf16":c,"feature":null,"max_stack":7,"storage":1,"bind":null,"trade_on_bind":null}],"extras":[]});
    let rows=catalog["equips"].as_array().unwrap().iter().enumerate().map(|(i,r)|json!({"source_index":i,"item_utf16":r["item_utf16"],"feature":r["feature"],"stats":vec![10*(i as i32+1);18],"max_durability":17,"weight_bits":"3ffe666666666666","armor_hp":23})).collect::<Vec<_>>();
    let stats = json!({"format_version":1,"source":"heroesContents.item-stats","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"rows":rows});
    (catalog, stats)
}
fn stats(v: &Value) -> Result<Package, vindictus_server::error::Error> {
    Package::read(serde_json::to_vec(v).unwrap().as_slice())
}
#[test]
fn stats_bind_to_the_same_effective_equipment_winner_and_unknown_classes_stay_absent() {
    let (c, s) = fixtures();
    let catalog = item_catalog::read_package(serde_json::to_vec(&c).unwrap().as_slice()).unwrap();
    let p = stats(&s).unwrap();
    let bound = p.bind(&catalog).unwrap();
    let bytes = |name: &str| {
        name.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let a = bytes("synthetic");
    let plain = bytes("plain");
    let stat = bound.get(Some(&a)).unwrap();
    assert_eq!(stat.fields(), [17, 1000, 0, 23]);
    assert_eq!(
        stat.get(Some(&"STR".encode_utf16().collect::<Vec<_>>())),
        30
    );
    let empty = bound.get(Some(&plain)).unwrap();
    assert_eq!(empty.fields(), [0; 4]);
    assert_eq!(empty.character_entries().count(), 0);
    assert!(bound.get(Some(&bytes("other"))).is_none());
    assert!(bound.get(None).is_none());
    assert!(bound.get(Some(&bytes("unknown"))).is_none());
    let report = p.inspect(&catalog).unwrap();
    assert_eq!(report.effective_equipment, 2);
    assert_eq!(report.effective_classes, 2);
    assert_eq!(report.classes_with_equipment_stats, 1);
    for field in ["database_sha256", "config_sha256"] {
        let mut wrong = s.clone();
        wrong[field] = json!("c".repeat(64));
        assert!(stats(&wrong).unwrap().bind(&catalog).is_err());
    }
    let mut wrong = s.clone();
    wrong["rows"][0]["feature"] = json!("off");
    assert!(stats(&wrong).unwrap().bind(&catalog).is_err());
    let mut wrong = s.clone();
    wrong["rows"][0]["item_utf16"] = json!(key("other"));
    assert!(stats(&wrong).unwrap().bind(&catalog).is_err());
    let mut wrong = s.clone();
    wrong["rows"].as_array_mut().unwrap().pop();
    assert!(stats(&wrong).unwrap().bind(&catalog).is_err());
}
#[test]
fn private_reader_refuses_missing_unknown_oversized_or_reordered_fields() {
    let (_, s) = fixtures();
    for field in [
        "source_index",
        "item_utf16",
        "feature",
        "stats",
        "max_durability",
        "weight_bits",
        "armor_hp",
    ] {
        let mut v = s.clone();
        v["rows"][0].as_object_mut().unwrap().remove(field);
        assert!(stats(&v).is_err());
    }
    for field in [
        "format_version",
        "source",
        "database_sha256",
        "config_sha256",
        "rows",
    ] {
        let mut v = s.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(stats(&v).is_err());
    }
    for value in [
        json!(null),
        json!([]),
        json!(vec![0; 17]),
        json!(vec![0; 19]),
        json!(vec![2147483648_i64; 18]),
    ] {
        let mut v = s.clone();
        v["rows"][0]["stats"] = value;
        assert!(stats(&v).is_err());
    }
    for bits in ["0", "zzzzzzzzzzzzzzzz", "00000000000000000"] {
        let mut v = s.clone();
        v["rows"][0]["weight_bits"] = json!(bits);
        assert!(stats(&v).is_err());
    }
    let mut v = s.clone();
    v["rows"].as_array_mut().unwrap().swap(0, 2);
    assert!(stats(&v).is_err());
    let mut v = s.clone();
    v["extra"] = json!(true);
    assert!(stats(&v).is_err());
    let mut v = s.clone();
    v["rows"][0]["extra"] = json!(true);
    assert!(stats(&v).is_err());
    let mut v = s.clone();
    v["rows"][0]["item_utf16"] = json!("00");
    assert!(stats(&v).is_err());
    assert!(Package::read(vec![32; 16 * 1024 * 1024 + 1].as_slice()).is_err());
}
#[test]
fn diagnostic_cli_uses_only_stdin_and_source_catalog_and_emits_no_content_rows() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let (c, s) = fixtures();
    let path = std::env::temp_dir().join(format!(
        "vindictus-item-stats-{}.json",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&path, serde_json::to_vec(&c).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["inspect-item-stats", "--input-stdin", "--catalog-file"])
        .arg(&path)
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&serde_json::to_vec(&s).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["source_catalog_matched"], true);
    assert_eq!(report["character_creation_available"], false);
    assert!(report.get("rows").is_none());
    assert!(
        !String::from_utf8(output.stdout)
            .unwrap()
            .contains("synthetic")
    );
}
