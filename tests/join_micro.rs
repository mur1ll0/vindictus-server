use vindictus_server::domain::{join_micro::Window, legacy_user_data::SqlDateTime2};
#[test]
fn join_micro_daily_window_matches_original_framework_constructor() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-join-micro.json")).unwrap();
    assert_eq!(f["x86"], f["x64"]);
    let cases = f["x86"].as_array().unwrap();
    assert_eq!(cases.len(), 128);
    let mut compared = 0;
    for c in cases {
        if c["last"] == "0" {
            continue;
        }
        let date = |i: usize| {
            SqlDateTime2::from_ticks(c["times"][i].as_str().unwrap().parse().unwrap()).unwrap()
        };
        let w = Window::reference_vm(date(2)).unwrap();
        assert_eq!(w.last().ticks().to_string(), c["last"].as_str().unwrap());
        assert_eq!(w.next().ticks().to_string(), c["next"].as_str().unwrap());
        let reset = w.needs_reset(date(0));
        assert_eq!(reset, c["clocks"].as_u64().unwrap() == 4);
        compared += 1;
    }
    assert!(compared > 40);
}
#[test]
fn join_micro_daily_equality_and_tick_before_use_previous_window() {
    let midnight = 639017856000000000i64;
    let boundary = midnight + 36_000_000_000;
    for (delta, shift) in [(-1, -864_000_000_000), (0, -864_000_000_000), (1, 0)] {
        let now = SqlDateTime2::from_ticks(boundary + delta).unwrap();
        let w = Window::reference_vm(now).unwrap();
        assert_eq!(w.last().ticks(), boundary + shift);
        assert_eq!(w.next().ticks() - w.last().ticks(), 864_000_000_000);
        assert!(!w.needs_reset(w.last()));
    }
}
