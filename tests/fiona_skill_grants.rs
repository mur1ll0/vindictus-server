use serde_json::Value;
use vindictus_server::{
    application::character_creation::EntityAuthority,
    domain::legacy_user_data::{SqlDateTime, SqlDateTime2},
    infrastructure::fiona_skill_grants,
};
mod fixture {
    include!("fixtures/initializer_profile_support.rs");
}
#[test]
fn first_fiona_grant_plan_matches_admitted_original_gives_and_shared_allocator() {
    let cases: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-skills.json")).unwrap();
    let mut admitted = 0;
    for c in cases["cases"].as_array().unwrap() {
        if ![7, 9].contains(&c["scenario"].as_i64().unwrap()) {
            continue;
        }
        admitted += 1;
        assert!(c["reason"].is_null());
        assert_eq!(c["allocations"], 1);
        assert_eq!(c["skill"]["current_ap"], 0);
        assert_eq!(c["skill"]["locked"], 0);
        let mut raw = fixture::synthetic();
        raw["skills"][0]["rank"] = c["rank"].clone();
        raw["skills"][0]["next_rank"] = if c["next_ap"].is_null() {
            Value::Null
        } else {
            serde_json::json!({"required_ap":c["next_ap"],"require_unlock":false})
        };
        let profile = fixture::read(&raw).unwrap();
        let mut authority = EntityAuthority::new(17);
        let now = SqlDateTime2::from_ticks(638000000000000000).unwrap();
        let cid = authority.allocate(now).unwrap().entity_time.ticks();
        let item = authority.allocate(now).unwrap().entity_time.ticks();
        assert!(item > cid);
        let batch = fiona_skill_grants::prepare(&profile, &mut authority, || now).unwrap();
        assert_eq!(batch.rows().len(), 1);
        let r = &batch.rows()[0];
        assert_eq!(r.id(), item + 65536);
        assert_eq!(authority.counter(), r.id());
        assert_eq!(i64::from(r.rank()), c["rank"].as_i64().unwrap());
        assert_eq!(
            i64::from(r.required_ap()),
            c["required_ap"].as_i64().unwrap()
        );
        assert!(r.item_callback());
        assert!(!r.stat_callback());
    }
    assert_eq!(admitted, 40);
}
#[test]
fn sql_server_datetime2_clock_matches_recovered_framework_conversion() {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/sql-character-log-clock.json")).unwrap();
    assert_eq!(f["civil_offset_minutes"], 0);
    assert_eq!(f["cases"].as_array().unwrap().len(), 256);
    for c in f["cases"].as_array().unwrap() {
        let h = c["source_binary"].as_str().unwrap();
        let mut bytes = [0; 8];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap();
        }
        let d = SqlDateTime::from_sql_binary(bytes).unwrap();
        assert_eq!(
            d.framework_date_time().unwrap().ticks().to_string(),
            c["converted_ticks"].as_str().unwrap()
        );
    }
}
