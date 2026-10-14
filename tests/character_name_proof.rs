#[path = "support/name_check_common.rs"]
mod common;
use common::*;
use vindictus_server::infrastructure::character_name_proof::NameProof;
#[test]
fn finite_name_proof_reads_measured_synthetic_values_and_refuses_profile_or_reference_corruption() {
    let content = content();
    let valid = value(&content);
    let _validated = proof(&content);
    for (key, bad) in [
        ("format_version", serde_json::json!(2)),
        ("candidate", serde_json::json!("not valid")),
        ("content_database_sha256", serde_json::json!("0".repeat(64))),
        ("original_config_sha256", serde_json::json!("0".repeat(64))),
        ("sql_collation", serde_json::json!("invented")),
        ("japanese", serde_json::json!(true)),
        ("naming_rule", serde_json::json!(".*")),
        ("max_bytes", serde_json::json!(21)),
        ("forbidden_filtered_rows", serde_json::json!(0)),
        ("forbidden_cultures", serde_json::json!(0)),
    ] {
        let mut modified = valid.clone();
        modified[key] = bad;
        assert!(
            NameProof::read(serde_json::to_vec(&modified).unwrap().as_slice(), &content).is_err()
        );
    }
    let mut contradict = valid.clone();
    contradict["references"][0]["equal"] = serde_json::json!(false);
    assert!(
        NameProof::read(
            serde_json::to_vec(&contradict).unwrap().as_slice(),
            &content
        )
        .is_err()
    );
    let mut malformed = valid.clone();
    malformed["references"][1]["utf16_hex"] = serde_json::json!("410");
    assert!(NameProof::read(serde_json::to_vec(&malformed).unwrap().as_slice(), &content).is_err());
    let oversized = vec![b' '; 1024 * 1024 + 1];
    assert!(NameProof::read(oversized.as_slice(), &content).is_err());
}
#[test]
fn recovered_name_error_labels_match_original_framework_cast_between_operation_enums() {
    use vindictus_server::infrastructure::character_name_check::NameCheckResult as R;
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-name-error-enum.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        for (code, result) in [
            (2, R::InvalidName),
            (3, R::DuplicatedName),
            (4, R::ReservedName),
            (5, R::ForbiddenName),
            (6, R::ChangedCharacterName),
        ] {
            let case = &profile["cases"][code];
            assert!(case["error"].is_null());
            assert_eq!(
                result.error_key().unwrap(),
                format!(
                    "GameUI_Heroes_Error_Create_Character_{}",
                    case["label"].as_str().unwrap()
                )
            );
        }
    }
    assert_eq!(R::Success.error_key(), None);
}
