mod support_fiona {
    include!("fixtures/fiona_inventory_support.rs");
}
use serde_json::{Value, json};
use support_fiona::*;
use vindictus_server::{
    application::{
        character_creation::EntityAuthority,
        fiona_inventory::{self, Inputs},
    },
    domain::{item_colors::ColorRandom, legacy_user_data::SqlDateTime2},
    infrastructure::{fiona_inventory_profile::Profile, item_catalog, item_colors, item_stats},
};
fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn read(v: &Value) -> Result<Profile, vindictus_server::error::Error> {
    Profile::read(bytes(v).as_slice())
}
#[test]
fn composition_binds_all_sources_groups_defaults_and_shares_character_item_authority() {
    let (c, s, h, p) = fixtures();
    let c = item_catalog::read_package(bytes(&c).as_slice()).unwrap();
    let s = item_stats::Package::read(bytes(&s).as_slice()).unwrap();
    let h = item_colors::Package::read(bytes(&h).as_slice()).unwrap();
    let p = read(&p).unwrap();
    let mut authority = EntityAuthority::new(17);
    let now = SqlDateTime2::from_ticks(638448480000000000).unwrap();
    let old = authority.allocate(now).unwrap().entity_time.ticks();
    let batch = fiona_inventory::compose(
        Inputs {
            class: 1,
            profile: &p,
            catalog: &c,
            stats: &s,
            colors: &h,
        },
        1,
        &mut authority,
        &mut ColorRandom::from_seed(0),
        &mut || Ok(now),
    )
    .unwrap();
    assert_eq!(batch.items().len(), 2);
    assert_eq!(batch.items()[0].count(), 5);
    assert_eq!(batch.items()[0].location().collection, 100);
    assert_eq!(batch.items()[1].location().collection, 0);
    assert!(batch.items()[0].id() > old);
    assert_eq!(authority.counter(), batch.items()[1].id());
    let e = batch.items()[1].equipment().unwrap();
    assert_eq!(e.colors, [0, 0xffffff, 0xffffff]);
    assert_eq!(e.bonus, -100);
    assert_eq!(e.reduced, 0);
    let report = fiona_inventory::inspect(&batch);
    assert_eq!(report.cash_items, 1);
    assert_eq!(report.bag_items, 1);
    assert!(!report.database_mutations);
}
#[test]
fn profile_refuses_missing_unknown_misaligned_and_unrecovered_rules() {
    let (c, s, h, p) = fixtures();
    let cat = item_catalog::read_package(bytes(&c).as_slice()).unwrap();
    let stats = item_stats::Package::read(bytes(&s).as_slice()).unwrap();
    let colors = item_colors::Package::read(bytes(&h).as_slice()).unwrap();
    let bound_s = stats.bind(&cat).unwrap();
    let bound_h = colors.bind(&cat).unwrap();
    for field in [
        "format_version",
        "source",
        "database_sha256",
        "config_sha256",
        "constants_sha256",
        "gold_utf16",
        "tircoin_utf16",
        "features",
        "classes",
        "requests",
        "variable_keys",
    ] {
        let mut v = p.clone();
        v.as_object_mut().unwrap().remove(field);
        assert!(read(&v).is_err());
    }
    for field in [
        "source_index",
        "item_utf16",
        "feature",
        "category_utf16",
        "unique",
        "expire_in",
    ] {
        let mut v = p.clone();
        v["classes"][0].as_object_mut().unwrap().remove(field);
        assert!(read(&v).is_err());
    }
    for (field, value) in [
        ("database_sha256", json!("d".repeat(64))),
        ("config_sha256", json!("d".repeat(64))),
    ] {
        let mut v = p.clone();
        v[field] = value;
        assert!(read(&v).unwrap().bind(&cat).is_err());
    }
    let mut v = p.clone();
    v["classes"][0]["feature"] = json!("off");
    assert!(read(&v).unwrap().bind(&cat).is_err());
    let mut v = p.clone();
    v["variable_keys"] = json!([key("synthetic_bag")]);
    assert!(
        read(&v)
            .unwrap()
            .requests(1, &cat, &bound_s, &bound_h)
            .is_err()
    );
    let mut v = p.clone();
    v["classes"][0]["category_utf16"] = json!(key("BRACELET"));
    assert!(
        read(&v)
            .unwrap()
            .requests(1, &cat, &bound_s, &bound_h)
            .is_err()
    );
    let mut v = p.clone();
    v["requests"][0]["item_utf16"] = json!(key("synthetic_bag[X:1]"));
    assert!(
        read(&v)
            .unwrap()
            .requests(1, &cat, &bound_s, &bound_h)
            .is_err()
    );
    let mut v = p.clone();
    v["extra"] = json!(1);
    assert!(read(&v).is_err());
    assert!(Profile::read(vec![32; 16 * 1024 * 1024 + 1].as_slice()).is_err());
}
#[test]
fn diagnostic_compares_only_private_inputs_and_exposes_counts_digest_without_database() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let (c, s, h, p) = fixtures();
    let root = std::env::temp_dir().join(format!("fiona-profile-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for (name, data) in [("catalog", &c), ("stats", &s), ("colors", &h)] {
        std::fs::write(root.join(name), bytes(data)).unwrap();
    }
    let mut child = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["inspect-fiona-inventory", "--input-stdin", "--catalog-file"])
        .arg(root.join("catalog"))
        .arg("--stats-file")
        .arg(root.join("stats"))
        .arg("--colors-file")
        .arg(root.join("colors"))
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes(&p)).unwrap();
    let out = child.wait_with_output().unwrap();
    for name in ["catalog", "stats", "colors"] {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    assert!(out.status.success());
    let text = String::from_utf8(out.stdout).unwrap();
    let report: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(report["items"], 2);
    assert_eq!(report["character_creation_available"], false);
    assert_eq!(report["database_mutations"], false);
    assert!(!text.contains("synthetic_bag"));
    assert!(!text.contains("item_utf16"));
}
