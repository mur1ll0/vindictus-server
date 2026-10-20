use vindictus_server::{
    domain::legacy_user_data::SqlDateTime2, infrastructure::minimum_character_list::delete_wait,
};
#[test]
fn summary_delete_wait_matches_original_framework_arithmetic_and_clock_reads() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-minimum-delete-wait.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        for c in profile["cases"].as_array().unwrap() {
            let anchor = 638448480000000000i64;
            let delta = c["delta"].as_str().unwrap().parse::<i64>().unwrap();
            let deleted = (!c["absent"].as_bool().unwrap())
                .then(|| SqlDateTime2::from_ticks(anchor - delta).unwrap());
            let mut reads = 0;
            let actual = delete_wait(c["delay"].as_i64().unwrap() as i32, deleted, || {
                reads += 1;
                Ok(
                    SqlDateTime2::from_ticks(if reads == 1 { anchor } else { anchor - delta })
                        .unwrap(),
                )
            })
            .unwrap();
            assert_eq!(actual, c["result"].as_i64().unwrap() as i32, "{c}");
            assert_eq!(reads, c["reads"].as_i64().unwrap());
        }
    }
}
