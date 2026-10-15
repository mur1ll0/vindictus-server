use serde::Deserialize;
use vindictus_server::domain::{
    item_time::{ItemTimeError, initialize_expiry, minimum_uptime, property_expiry},
    legacy_user_data::SqlDateTime2,
};
#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Input {
    expire_present: bool,
    expire_value: Option<String>,
    expiry: Option<i64>,
    min_present: bool,
    min_value: Option<String>,
    expire_in: Option<i32>,
    now: i64,
}
#[derive(Deserialize)]
struct Case {
    input: Input,
    error: Option<String>,
    phase: String,
    property_expiry: Option<i64>,
    item_expiry: Option<i64>,
    has_expire: bool,
    min_value: Option<String>,
    clock_calls: usize,
}
fn date(value: i64) -> SqlDateTime2 {
    SqlDateTime2::from_ticks(value).unwrap()
}
#[test]
fn temporal_source_matrix_matches_framework_on_both_architectures() {
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/framework-item-time.json")).unwrap();
    assert_eq!(fixture.cases.len(), 1848);
    for (index, case) in fixture.cases.into_iter().enumerate() {
        let input = case.input;
        let expire: Option<Vec<u16>> = input
            .expire_value
            .as_ref()
            .map(|v| v.encode_utf16().collect());
        let uptime: Option<Vec<u16>> = input.min_value.as_ref().map(|v| v.encode_utf16().collect());
        let mut expiry = input.expiry.map(date);
        let mut has_expire = input.expire_present;
        let mut min_value = input.min_value.clone();
        let mut item_expiry = None;
        let mut calls = 0;
        let mut clock = || {
            let now = date(input.now + 101 * calls as i64);
            calls += 1;
            Ok(now)
        };
        let mut phase = "property";
        let result = (|| {
            let update = property_expiry(expiry, has_expire.then_some(expire.as_deref()))?;
            expiry = update.expiry;
            has_expire &= !update.remove_expire_attribute;
            if let Some(value) =
                minimum_uptime(input.min_present.then_some(uptime.as_deref()), &mut clock)?
            {
                min_value = Some(String::from_utf16(&value).unwrap());
            }
            phase = "initialize";
            // The source assigns the property date before sampling/validating ExpireIn.
            item_expiry = expiry;
            let update = initialize_expiry(expiry, has_expire, input.expire_in, &mut clock)?;
            item_expiry = update.expiry;
            has_expire &= !update.remove_expire_attribute;
            phase = "complete";
            Ok::<_, ItemTimeError>(())
        })();
        let error = result.err().map(|e| match e {
            ItemTimeError::NullExpire => "NullReferenceException",
            ItemTimeError::DateRange => "ArgumentOutOfRangeException",
            _ => panic!("unexpected bounded fixture error"),
        });
        assert_eq!(error, case.error.as_deref(), "error case {index}");
        assert_eq!(phase, case.phase, "phase case {index}");
        assert_eq!(
            expiry.map(SqlDateTime2::ticks),
            case.property_expiry,
            "property case {index}"
        );
        assert_eq!(
            item_expiry.map(SqlDateTime2::ticks),
            case.item_expiry,
            "item case {index}"
        );
        assert_eq!(has_expire, case.has_expire, "attribute case {index}");
        assert_eq!(min_value, case.min_value, "uptime case {index}");
        assert_eq!(calls, case.clock_calls, "clock case {index}");
    }
}
#[test]
fn unavailable_clocks_are_lazy_errors_and_bounds_never_produce_partial_updates() {
    let mut calls = 0;
    let mut unavailable = || {
        calls += 1;
        Err(ItemTimeError::Clock)
    };
    assert_eq!(
        initialize_expiry(Some(date(1)), true, None, &mut unavailable)
            .unwrap()
            .expiry,
        Some(date(1))
    );
    assert!(minimum_uptime(None, &mut unavailable).unwrap().is_none());
    assert!(
        minimum_uptime(Some(Some(&[32])), &mut unavailable)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        minimum_uptime(Some(None), &mut unavailable),
        Err(ItemTimeError::Clock)
    );
    assert_eq!(
        initialize_expiry(Some(date(1)), false, Some(0), &mut unavailable),
        Err(ItemTimeError::Clock)
    );
    assert_eq!(
        property_expiry(None, Some(Some(&vec![49; 1025]))),
        Err(ItemTimeError::Bound)
    );
    assert_eq!(
        minimum_uptime(Some(Some(&vec![49; 1025])), &mut unavailable),
        Err(ItemTimeError::Bound)
    );
    assert_eq!(calls, 2);
    let mut zero = || Ok(date(0));
    assert_eq!(
        initialize_expiry(None, false, Some(-1), &mut zero),
        Err(ItemTimeError::DateRange)
    );
    let mut max = || Ok(date(SqlDateTime2::MAX));
    assert_eq!(
        initialize_expiry(None, false, Some(1), &mut max),
        Err(ItemTimeError::DateRange)
    );
}
