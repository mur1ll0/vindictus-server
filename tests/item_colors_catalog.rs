use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vindictus_server::infrastructure::{item_catalog, item_colors::Package};
fn key(s: &str) -> String {
    s.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn fixtures() -> (Value, Value) {
    let a = key("synthetic");
    let plain = key("plain");
    let m = key("material");
    let equips = json!([{"item_utf16":a,"feature":"on"},{"item_utf16":a,"feature":"off"},{"item_utf16":a,"feature":"on"}]);
    let catalog = json!({"format_version":1,"source":"heroesContents.item-catalog-projection","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"region":"STUDY-A","override_region":null,"equip_rows":3,"class_rows":2,"extra_rows":0},"features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"StoryTabInInventory","value":""},{"key":"on","value":"1.1"},{"key":"off","value":null}],"equips":equips,"classes":[{"item_utf16":a,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null},{"item_utf16":plain,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null}],"extras":[]});
    let rows=catalog["equips"].as_array().unwrap().iter().enumerate().map(|(i,r)|json!({"source_index":i,"item_utf16":r["item_utf16"],"feature":r["feature"],"materials":if i==2{json!([m,null,m])}else{json!([null,null,null])}})).collect::<Vec<_>>();
    let colors = json!({"format_version":1,"source":"heroesContents.item-colors","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"rows":rows,"materials":[{"name_utf16":m,"raw_hex":"010203ff","sha256":format!("{:x}",Sha256::digest([1,2,3,255]))}]});
    (catalog, colors)
}
fn read(v: &Value) -> Result<Package, vindictus_server::error::Error> {
    Package::read(serde_json::to_vec(v).unwrap().as_slice())
}
#[test]
fn colors_bind_to_effective_duplicate_winner_and_require_same_source_snapshot() {
    let (c, p) = fixtures();
    let catalog = item_catalog::read_package(serde_json::to_vec(&c).unwrap().as_slice()).unwrap();
    let package = read(&p).unwrap();
    let bound = package.bind(&catalog).unwrap();
    let bytes = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>()
    };
    let names = bound.get(Some(&bytes("synthetic"))).unwrap();
    assert!(names[1].is_none());
    assert_eq!(
        names[0].unwrap(),
        "material".encode_utf16().collect::<Vec<_>>()
    );
    assert!(bound.get(Some(&bytes("plain"))).is_none());
    assert!(bound.get(Some(&bytes("unknown"))).is_none());
    assert!(bound.get(None).is_none());
    assert_eq!(
        bound
            .materials()
            .get(names[0].unwrap())
            .unwrap()
            .unwrap()
            .colors(),
        [0x010203]
    );
    let report = serde_json::to_value(package.inspect(&catalog).unwrap()).unwrap();
    assert_eq!(report["effective_equipment"], 1);
    assert_eq!(report["total_colors"], 1);
    for field in ["database_sha256", "config_sha256"] {
        let mut bad = p.clone();
        bad[field] = json!("c".repeat(64));
        assert!(read(&bad).unwrap().bind(&catalog).is_err());
    }
    let mut bad = p.clone();
    bad["rows"][0]["feature"] = json!("off");
    assert!(read(&bad).unwrap().bind(&catalog).is_err());
    let mut bad = p.clone();
    bad["rows"][0]["item_utf16"] = json!(key("other"));
    assert!(read(&bad).unwrap().bind(&catalog).is_err());
    let mut bad = p.clone();
    bad["rows"].as_array_mut().unwrap().pop();
    assert!(read(&bad).unwrap().bind(&catalog).is_err());
}
#[test]
fn private_snapshot_requires_fields_hashes_alignment_material_presence_and_bounds() {
    let (_, p) = fixtures();
    for field in [
        "format_version",
        "source",
        "database_sha256",
        "config_sha256",
        "rows",
        "materials",
    ] {
        let mut bad = p.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(read(&bad).is_err());
    }
    for field in ["source_index", "item_utf16", "feature", "materials"] {
        let mut bad = p.clone();
        bad["rows"][0].as_object_mut().unwrap().remove(field);
        assert!(read(&bad).is_err());
    }
    for field in ["name_utf16", "raw_hex", "sha256"] {
        let mut bad = p.clone();
        bad["materials"][0].as_object_mut().unwrap().remove(field);
        assert!(read(&bad).is_err());
    }
    for (field, value) in [
        ("raw_hex", json!("00")),
        ("raw_hex", json!("gg")),
        ("name_utf16", json!("00")),
        ("sha256", json!("c".repeat(64))),
    ] {
        let mut bad = p.clone();
        bad["materials"][0][field] = value;
        assert!(read(&bad).is_err());
    }
    let mut bad = p.clone();
    bad["rows"][0]["source_index"] = json!(1);
    assert!(read(&bad).is_err());
    let mut bad = p.clone();
    bad["rows"][0]["materials"] = json!([null]);
    assert!(read(&bad).is_err());
    let mut bad = p.clone();
    bad["rows"][0]["materials"] = json!([key("unbound"), null, null]);
    assert!(read(&bad).is_err());
    let mut bad = p.clone();
    bad["extra"] = json!(true);
    assert!(read(&bad).is_err());
    let mut bad = p.clone();
    let duplicate = bad["materials"][0].clone();
    bad["materials"].as_array_mut().unwrap().push(duplicate);
    assert!(read(&bad).is_err());
    assert!(Package::read(vec![32; 16 * 1024 * 1024 + 1].as_slice()).is_err());
    let mut missing = p.clone();
    missing["materials"][0]["raw_hex"] = Value::Null;
    missing["materials"][0]["sha256"] = Value::Null;
    assert!(read(&missing).is_ok());
}
#[test]
fn colors_diagnostic_is_stdin_only_without_database_and_outputs_no_names_or_assets() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let (c, p) = fixtures();
    let path = std::env::temp_dir().join(format!(
        "vindictus-colors-catalog-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&c).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["inspect-item-colors", "--input-stdin", "--catalog-file"])
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
        .write_all(&serde_json::to_vec(&p).unwrap())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["source_catalog_matched"], true);
    assert_eq!(report["database_mutations"], false);
    assert_eq!(report["character_creation_available"], false);
    for secret in [
        "synthetic",
        "material\"",
        "010203ff",
        "raw_hex",
        "item_utf16",
    ] {
        assert!(!text.contains(secret));
    }
}
