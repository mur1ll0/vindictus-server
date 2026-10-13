use serde_json::{Value, json};
use vindictus_server::{
    domain::{home_cafe_bonus::*, legacy_user_data::SqlDateTime},
    infrastructure::home_cafe_bonus::read_snapshot,
};
#[test]
fn home_cafe_account_preference_matches_all_original_sql_assignments() {
    let f: Value = serde_json::from_str(include_str!("fixtures/sql-home-cafe-bonus.json")).unwrap();
    assert_eq!(f["cases"].as_array().unwrap().len(), 64);
    for c in f["cases"].as_array().unwrap() {
        let mask = c["mask"].as_u64().unwrap();
        let expiry =
            SqlDateTime::from_units300(c["row_expiry_units300"].as_str().unwrap().parse().unwrap())
                .unwrap();
        let mut rows = Vec::new();
        for (bit, bonus) in [(1, 3), (2, 4), (4, 2)] {
            if mask & bit != 0 {
                rows.push(HomeCafeBonus {
                    uid: 44,
                    cid: 0,
                    bonus_type: bonus,
                    expired_time: expiry,
                })
            }
        }
        for (uid, cid, bonus) in [(45, 0, 3), (44, 7, 3), (44, 0, 1)] {
            rows.push(HomeCafeBonus {
                uid,
                cid,
                bonus_type: bonus,
                expired_time: expiry,
            })
        }
        validate(&rows).unwrap();
        let initial = c["initial_bonus_type"].as_u64().map(|x| x as u8);
        let result = c["bonus_type"].as_u64().map(|x| x as u8);
        assert_eq!(account_bonus(&rows, 44, initial), result);
        assert!(c["expired_output_unchanged"].as_bool().unwrap());
    }
}
#[test]
fn home_cafe_private_snapshot_retains_full_signed_keys_dates_and_rejects_malformed_exports() {
    let raw = include_str!("fixtures/synthetic-home-cafe-bonus.json");
    let rows = read_snapshot(raw.as_bytes()).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (
            rows[0].uid,
            rows[0].cid,
            rows[0].bonus_type,
            rows[0].expired_time.units300()
        ),
        (i32::MAX, i64::MAX, 255, SqlDateTime::MIN)
    );
    assert_eq!(
        (
            rows[1].uid,
            rows[1].cid,
            rows[1].bonus_type,
            rows[1].expired_time.units300()
        ),
        (i32::MIN, i64::MIN, 0, SqlDateTime::MAX)
    );
    for field in ["uid", "cid", "bonus_type", "expired_time_units300"] {
        let mut v: Value = serde_json::from_str(raw).unwrap();
        v["home_cafe_bonus"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    }
    for (field, bad) in [
        ("cid", json!("01")),
        ("cid", json!(i64::MAX)),
        ("bonus_type", json!(256)),
        ("expired_time_units300", json!("76683386880000")),
        ("expired_time_units300", Value::Null),
    ] {
        let mut v: Value = serde_json::from_str(raw).unwrap();
        v["home_cafe_bonus"][0][field] = bad;
        assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    }
    let mut v: Value = serde_json::from_str(raw).unwrap();
    let r = v["home_cafe_bonus"][0].clone();
    v["home_cafe_bonus"].as_array_mut().unwrap().push(r.clone());
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    v["home_cafe_bonus"] = json!(vec![r; MAX_ROWS + 1]);
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
    assert!(read_snapshot(vec![b' '; 2 * 1024 * 1024 + 1].as_slice()).is_err());
    assert!(
        read_snapshot(br#"{"format_version":1,"source":"other","home_cafe_bonus":[]}"#.as_slice())
            .is_err()
    );
    let mut v: Value = serde_json::from_str(raw).unwrap();
    v["extra"] = json!(0);
    assert!(read_snapshot(serde_json::to_vec(&v).unwrap().as_slice()).is_err());
}
#[test]
fn frontend_feature_pairs_match_original_framework_across_architectures_and_cultures() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-frontend-features.json")).unwrap();
    let mut count = 0;
    for p in f["profiles"].as_array().unwrap() {
        for c in p["cases"].as_array().unwrap() {
            let policy = vindictus_server::domain::feature_policy::FeaturePolicy::new([
                (
                    String::from("CurrentVer"),
                    Some(c["current"].as_str().unwrap().to_owned()),
                ),
                (
                    String::from("HomeCafeBonus"),
                    Some(c["value"].as_str().unwrap().to_owned()),
                ),
            ])
            .unwrap();
            assert_eq!(
                policy.is_enabled(Some("HomeCafeBonus")).unwrap(),
                c["enabled"].as_bool().unwrap()
            );
            count += 1
        }
    }
    assert_eq!(count, 20);
}
