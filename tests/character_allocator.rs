use serde_json::Value;
use vindictus_server::domain::{character_allocator::*, legacy_user_data::SqlDateTime2};
fn integer(v: &Value) -> i64 {
    v.as_str().unwrap().parse().unwrap()
}
fn time(v: &Value) -> SqlDateTime2 {
    SqlDateTime2::from_ticks(integer(v)).unwrap()
}
#[test]
fn entity_allocator_matches_original_mutation_delays_masking_and_date_range_failures() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-character-allocator.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["cid"].as_array().unwrap().len(), 252);
        for case in profile["cid"].as_array().unwrap() {
            let mut old = integer(&case["previous"]);
            let result = make_entity_id(
                &mut old,
                case["service_id"].as_i64().unwrap() as i32,
                time(&case["now_ticks"]),
            );
            assert_eq!(old, integer(&case["after"]));
            if case["error"].is_null() {
                let value = result.unwrap();
                assert_eq!(value.entity_time.ticks(), integer(&case["result_ticks"]));
                assert_eq!(value.delay_ticks, integer(&case["delay_ticks"]));
            } else {
                assert!(result.is_err());
                assert_eq!(case["error"], "ArgumentOutOfRangeException");
            }
        }
    }
}
#[test]
fn character_serial_allocator_matches_both_clock_samples_signed_overflow_and_server_code_branches()
{
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-character-allocator.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        assert_eq!(profile["sn"].as_array().unwrap().len(), 560);
        for case in profile["sn"].as_array().unwrap() {
            let now = time(&case["now_ticks"]);
            let server = case["server_code"].as_i64().unwrap() as i32;
            let value = if case["first_now_ticks"].is_null() {
                next_character_serial(now, server, case["previous"].as_i64().unwrap() as i32)
            } else {
                first_character_serial(time(&case["first_now_ticks"]), now, server)
            };
            assert_eq!(value.serial, case["result"].as_i64().unwrap() as i32);
            assert_eq!(value.sampled_at, now);
            assert_eq!(value.delay_ticks, integer(&case["delay_ticks"]));
        }
        assert_eq!(profile["seconds"].as_array().unwrap().len(), 39);
        for case in profile["seconds"].as_array().unwrap() {
            assert_eq!(
                utc_unix_seconds(time(&case["utc_ticks"])),
                case["seconds"].as_i64().unwrap() as i32
            );
        }
    }
}
