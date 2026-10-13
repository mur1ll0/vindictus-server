use serde_json::{Value, json};
use vindictus_server::domain::legacy_user_data::SqlDateTime2;
use vindictus_server::infrastructure::{
    lab_clock::*,
    lab_runtime_config::*,
    reward_policy::{RewardPolicyPackage, read_policy},
};
const REQUIRED: &[&str] = &[
    "NGM_disable",
    "zhCN",
    "PassportImmigrator",
    "SecondPassword",
    "EssentialSecondPassword",
    "SecondPasswordClear",
    "SecondPassword_UsingSHA",
    "SecondPassword_IP_Limit",
    "MaxCharacterSlot",
    "UserCareSystem",
    "DisableInGameCheat",
    "DisableCharacterSlotCheat",
    "HomeCafeBonus",
    "CafeBonus",
    "CashShop",
    "FraudDetector",
    "NexonGameSecurity",
    "XignCode",
    "OldNexonIDProtect",
    "DisableLogging",
    "AsynchronousLogging",
    "HackshieldOff",
    "NotifyBanReason",
    "IPLockTime",
];
fn package_value() -> Value {
    let mut v: Value =
        serde_json::from_str(include_str!("fixtures/synthetic-reward-policy.json")).unwrap();
    for k in REQUIRED {
        v["features"].as_array_mut().unwrap().push(json!({"key":k,"value":if *k=="NGM_disable"||*k=="SecondPassword" {Some("1")} else {None}}));
    }
    v
}
fn package() -> RewardPolicyPackage {
    read_policy(serde_json::to_vec(&package_value()).unwrap().as_slice()).unwrap()
}
fn config() -> Value {
    json!({"format_version":1,"bind":"127.0.0.1:27015","framework_architecture":"x86","ascii_culture":"invariant",
 "content_database_sha256":"a".repeat(64),"original_config_sha256":"b".repeat(64),"region":"SYNTHETIC","override_region":null,
 "game_code":1,"server_code":1,"current_version":"3.09.00","service_id":42,"channel_code":0,"civil_offset_seconds":28800,
 "ping_pong":false,"allowed_passports":["synthetic_lab_a"]})
}
fn parse(v: &Value, p: &RewardPolicyPackage) -> Result<LabRuntimeConfig, RuntimeConfigError> {
    LabRuntimeConfig::read(serde_json::to_vec(v).unwrap().as_slice(), p)
}
#[test]
fn allowlist_requires_exact_full_passport_with_verified_ascii_case_and_explicit_profile() {
    let p = package();
    let c = parse(&config(), &p).unwrap();
    assert!(c.permits(Some("SYNTHETIC_LAB_A")));
    assert!(!c.permits(None));
    assert!(!c.permits(Some("synthetic_lab_a ")));
    assert!(!c.permits(Some("synthetic_lab_a_extra")));
    assert!(!c.permits(Some("synthetic_lab_é")));
    assert_eq!(c.bind(), "127.0.0.1:27015".parse().unwrap());
    assert_eq!(c.civil_offset_seconds(), 28800);
    assert!(!c.ping_pong());
    assert_eq!(c.scope().service_id(), 42);
    assert_eq!(c.scope().channel_code(), 0);
}
#[test]
fn config_refuses_nonloopback_missing_unknown_ambiguous_and_oversized_inputs_without_private_errors()
 {
    let p = package();
    for (key, value) in [
        ("bind", json!("0.0.0.0:27015")),
        ("bind", json!("127.0.0.1:0")),
        ("format_version", json!(2)),
        ("framework_architecture", json!("randomized")),
        ("ascii_culture", json!("tr-TR")),
        ("civil_offset_seconds", json!(50401)),
        ("channel_code", json!(256)),
        ("content_database_sha256", json!("a")),
        ("original_config_sha256", json!("c".repeat(64))),
        ("region", json!("OTHER")),
        ("override_region", json!("")),
        ("game_code", json!(2)),
        ("server_code", json!(2)),
        ("current_version", json!("3.09.01")),
        ("permissions", json!("cheat")),
        ("allowed_passports", json!([])),
        ("allowed_passports", json!(["a", "A"])),
        ("allowed_passports", json!([" "])),
        ("allowed_passports", json!(["synthetic\nsecret"])),
        ("allowed_passports", json!(["é"])),
        (
            "allowed_passports",
            json!([
                "abcdefghijklmnopqrstuvwx-one",
                "abcdefghijklmnopqrstuvwx-two"
            ]),
        ),
    ] {
        let mut v = config();
        v[key] = value;
        let e = parse(&v, &p).err().unwrap();
        assert!(!e.to_string().contains("synthetic"));
        assert!(!format!("{e:?}").contains("secret"));
    }
    for key in config().as_object().unwrap().keys() {
        let mut v = config();
        v.as_object_mut().unwrap().remove(key);
        assert!(parse(&v, &p).is_err(), "missing {key}");
    }
    let mut v = config();
    v["allowed_passports"] = json!((0..9).map(|i| format!("lab_{i}")).collect::<Vec<_>>());
    assert!(parse(&v, &p).is_err());
    let mut v = config();
    v["allowed_passports"] = json!(["a".repeat(1025)]);
    assert!(parse(&v, &p).is_err());
    assert!(LabRuntimeConfig::read(vec![b' '; MAX_CONFIG_BYTES + 1].as_slice(), &p).is_err());
    for k in REQUIRED {
        let mut v = package_value();
        v["features"]
            .as_array_mut()
            .unwrap()
            .retain(|f| f["key"] != *k);
        let p = read_policy(serde_json::to_vec(&v).unwrap().as_slice()).unwrap();
        assert!(parse(&config(), &p).is_err(), "undeclared {k}");
    }
    let mut v = package_value();
    for row in v["features"].as_array_mut().unwrap() {
        if row["key"] == "NGM_disable" {
            row["value"] = json!("");
        }
    }
    assert!(
        parse(
            &config(),
            &read_policy(serde_json::to_vec(&v).unwrap().as_slice()).unwrap()
        )
        .is_err()
    );
}
#[test]
fn provenance_fingerprint_binds_runtime_and_original_manifest_but_survives_subset_and_allowlist_growth()
 {
    let p = package();
    let base = parse(&config(), &p).unwrap();
    for (key, value) in [
        ("framework_architecture", json!("x64")),
        ("ascii_culture", json!("en-US")),
        ("service_id", json!(-7)),
        ("channel_code", json!(255)),
        ("civil_offset_seconds", json!(0)),
    ] {
        let mut v = config();
        v[key] = value;
        assert_ne!(
            base.scope().fingerprint(),
            parse(&v, &p).unwrap().scope().fingerprint()
        );
    }
    let mut v = config();
    v["bind"] = json!("[::1]:27016");
    v["allowed_passports"] = json!(["synthetic_lab_a", "synthetic_lab_b"]);
    assert_eq!(
        base.scope().fingerprint(),
        parse(&v, &p).unwrap().scope().fingerprint()
    );
    let mut value = package_value();
    value["features"]
        .as_array_mut()
        .unwrap()
        .push(json!({"key":"additional_source_declaration","value":null}));
    let p = read_policy(serde_json::to_vec(&value).unwrap().as_slice()).unwrap();
    assert_eq!(
        base.scope().fingerprint(),
        parse(&config(), &p).unwrap().scope().fingerprint()
    );
    for key in [
        "database_sha256",
        "config_sha256",
        "region",
        "override_region",
        "game_code",
        "server_code",
    ] {
        let mut value = package_value();
        let mut v = config();
        let (new, rawkey) = match key {
            "database_sha256" => (json!("c".repeat(64)), "content_database_sha256"),
            "config_sha256" => (json!("d".repeat(64)), "original_config_sha256"),
            "region" => (json!("OTHER"), "region"),
            "override_region" => (json!("OTHER"), "override_region"),
            "game_code" => (json!(2), "game_code"),
            _ => (json!(3), "server_code"),
        };
        value["manifest"][key] = new.clone();
        v[rawkey] = new;
        let p = read_policy(serde_json::to_vec(&value).unwrap().as_slice()).unwrap();
        assert_ne!(
            base.scope().fingerprint(),
            parse(&v, &p).unwrap().scope().fingerprint()
        );
    }
}
#[test]
fn unix_and_civil_ticks_match_framework_architectures_including_date_and_offset_boundaries() {
    let v: Value = serde_json::from_str(include_str!("fixtures/framework-lab-clock.json")).unwrap();
    assert_eq!(v["cases"].as_array().unwrap().len(), 140);
    assert_eq!(v["disagreements"], 0);
    for case in v["cases"].as_array().unwrap() {
        let sec = case["seconds"].as_str().unwrap().parse().unwrap();
        let ns = case["nanoseconds"].as_u64().unwrap() as u32;
        let utc = unix_ticks(sec, ns);
        if !case["utc_valid"].as_bool().unwrap() {
            assert!(utc.is_err());
            continue;
        }
        let utc = utc.unwrap();
        assert_eq!(utc.ticks().to_string(), case["utc_ticks"].as_str().unwrap());
        let local = civil_from_utc(utc, case["offset_seconds"].as_i64().unwrap() as i32);
        match case["civil_ticks"].as_str() {
            Some(expected) => assert_eq!(local.unwrap().ticks().to_string(), expected),
            None => assert!(local.is_err()),
        }
    }
    for sec in [i64::MIN, i64::MAX, -62135596801, 253402300800] {
        assert!(unix_ticks(sec, 0).is_err());
    }
    assert!(unix_ticks(0, 1_000_000_000).is_err());
    assert!(civil_from_utc(SqlDateTime2::from_ticks(0).unwrap(), -1).is_err());
    assert!(civil_from_utc(SqlDateTime2::from_ticks(SqlDateTime2::MAX).unwrap(), 1).is_err());
    assert!(civil_from_utc(unix_ticks(0, 0).unwrap(), -50401).is_err());
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let sample = utc_now().unwrap();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    assert!(
        sample.ticks()
            >= unix_ticks(before.as_secs() as i64, before.subsec_nanos())
                .unwrap()
                .ticks()
    );
    assert!(
        sample.ticks()
            <= unix_ticks(after.as_secs() as i64, after.subsec_nanos())
                .unwrap()
                .ticks()
    );
}
