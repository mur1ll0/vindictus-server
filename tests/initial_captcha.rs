use serde_json::{Value, json};
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    initial_captcha::{ConstructorError, Loaded, Source},
    initial_social,
    legacy_user_data::{SqlDateTime, SqlDateTime2},
};
fn social(uid: i32, cid: i64) -> initial_social::Loaded {
    let policy = FeaturePolicy::with_recorded_comparisons(
        [
            ("CurrentVer".into(), Some("a".into())),
            ("InGameGuild".into(), Some("on".into())),
        ],
        [("a".into(), "on".into(), true)],
    )
    .unwrap();
    initial_social::Loaded::load(uid, cid, &policy).unwrap()
}

fn date(v: &Value) -> Option<SqlDateTime> {
    v.as_str()
        .map(|v| SqlDateTime::from_units300(v.parse().unwrap()).unwrap())
}
fn source(v: &Value) -> Source {
    Source {
        punished: v["punished"].as_u64().map(|v| v as u8),
        failure: v["failure"].as_u64().map(|v| v as u8),
        last_failure: date(&v["last_failure_units300"]),
        last_succeed: date(&v["last_succeed_units300"]),
        condition: v["condition"].as_u64().map(|v| v as u8),
    }
}
fn ticks(v: Option<SqlDateTime2>) -> Option<String> {
    v.map(|v| v.ticks().to_string())
}
fn units(v: Option<SqlDateTime2>) -> Option<String> {
    v.map(|v| v.sql_parameter().unwrap().units300().to_string())
}
fn compare(arch: &str) {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-captcha.json")).unwrap();
    let cases = f[arch].as_array().unwrap();
    assert_eq!(cases.len(), 128);
    for c in cases {
        let query = c["query_mode"].as_i64().unwrap();
        if query == 7 {
            assert!(c["loaded"].is_null());
            continue;
        }
        let uid = c["uid"].as_i64().unwrap() as i32;
        let cid = c["cid"].as_str().unwrap().parse().unwrap();
        let parent = social(uid, cid);
        let row = if query < 4 {
            Some(source(&c["row"]))
        } else {
            None
        };
        let mut clocks = 0;
        let result = Loaded::construct(&parent, row, || {
            clocks += 1;
            if c["clock_fail"] == true {
                Err(ConstructorError::Clock)
            } else {
                Ok(
                    SqlDateTime2::from_ticks(c["now_ticks"].as_str().unwrap().parse().unwrap())
                        .unwrap(),
                )
            }
        });
        assert_eq!(
            clocks,
            c["trace"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| **v == "clock")
                .count()
        );
        if c["loaded"].is_null() {
            assert!(matches!(result, Err(ConstructorError::Clock)));
            continue;
        }
        let r = result.unwrap();
        let observed = json!({"uid":r.uid(),"cid":r.cid().to_string(),"punished":r.punished(),"failure":r.failure(),"last_failure_ticks":ticks(r.last_failure()),"last_succeed_ticks":ticks(r.last_succeed()),"force":r.force(),"pool_count":r.pool_count(),"timeout":r.timeout_schedule(),"refresh_count":r.refresh_count(),"last_check":ticks(r.last_check()),"last_refresh":ticks(r.last_refresh()),"parent_matches":r.matches_social(&parent)});
        assert_eq!(observed, c["loaded"]);
        if !c["payload"].is_null() {
            assert_eq!(
                json!({"uid":r.uid(),"punished":r.punished(),"failure":r.failure(),"last_failure_units300":units(r.last_failure()),"last_succeed_units300":units(r.last_succeed()),"condition":u8::from(r.force())}),
                c["payload"]
            );
        }
        assert_eq!(c["caught"], json!(if c["save_mode"] == 0 { 0 } else { 1 }));
    }
}
#[test]
fn captcha_matches_framework_x86() {
    compare("x86")
}
#[test]
fn captcha_matches_framework_x64() {
    compare("x64")
}
#[test]
fn captcha_empty_row_never_reads_clock_and_keeps_exact_shared_context() {
    let p = social(31, 1);
    let r = Loaded::construct(&p, None, || panic!("unexpected clock")).unwrap();
    assert!(r.matches_social(&p));
    assert!(!r.matches_social(&social(31, 1)));
    assert_eq!(
        (r.punished(), r.failure(), r.force(), r.pool_count()),
        (0, 0, false, 0)
    );
}
