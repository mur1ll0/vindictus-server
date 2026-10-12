use serde_json::Value;
use vindictus_server::domain::{
    item_argument::{self, ArgumentContext, ArgumentDateTime, ArgumentError, DateTimeKind},
    item_class_ex,
    legacy_user_data::LegacyBytes,
};

fn date(value: &Value) -> ArgumentDateTime {
    let kind = match value["kind"].as_str().unwrap() {
        "Utc" => DateTimeKind::Utc,
        "Local" => DateTimeKind::Local,
        "Unspecified" => DateTimeKind::Unspecified,
        _ => panic!("unexpected fixture kind"),
    };
    ArgumentDateTime::new(value["ticks"].as_str().unwrap().parse().unwrap(), kind).unwrap()
}
struct ObservedContext {
    parsed: Option<ArgumentDateTime>,
    suffix: Option<bool>,
    clock: ArgumentDateTime,
    offset_minutes: i64,
    calls: Vec<&'static str>,
    unavailable: bool,
}
impl ArgumentContext for ObservedContext {
    fn parse_date(&mut self, _: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError> {
        self.calls.push("date");
        if self.unavailable {
            return Err(ArgumentError::ProfileUnavailable);
        }
        Ok(self.parsed)
    }
    fn ends_with_seconds(&mut self, _: &[u16]) -> Result<bool, ArgumentError> {
        self.calls.push("suffix");
        self.suffix.ok_or(ArgumentError::ProfileUnavailable)
    }
    fn now_local(&mut self) -> Result<ArgumentDateTime, ArgumentError> {
        self.calls.push("clock");
        Ok(self.clock)
    }
    fn local_to_utc(&mut self, local: ArgumentDateTime) -> Result<ArgumentDateTime, ArgumentError> {
        self.calls.push("utc");
        ArgumentDateTime::new(
            (local.ticks() - self.offset_minutes * 60 * 10_000_000)
                .clamp(0, 3_155_378_975_999_999_999),
            DateTimeKind::Utc,
        )
    }
}
fn context() -> ObservedContext {
    ObservedContext {
        parsed: None,
        suffix: Some(true),
        clock: ArgumentDateTime::new(621_355_968_000_000_001, DateTimeKind::Local).unwrap(),
        offset_minutes: 0,
        calls: Vec::new(),
        unavailable: false,
    }
}
#[test]
fn temporal_decisions_ticks_and_lazy_calls_match_recovered_methods_in_thirty_profiles() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-item-argument.json")).unwrap();
    for arch in fixture["profiles"].as_array().unwrap() {
        for profile in arch["profiles"].as_array().unwrap() {
            for (index, case) in profile["cases"].as_array().unwrap().iter().enumerate() {
                let input = case["input_hex"].as_str().map(|hex| {
                    (0..hex.len())
                        .step_by(4)
                        .map(|i| {
                            u16::from_le_bytes([
                                u8::from_str_radix(&hex[i..i + 2], 16).unwrap(),
                                u8::from_str_radix(&hex[i + 2..i + 4], 16).unwrap(),
                            ])
                        })
                        .collect::<Vec<_>>()
                });
                let mut ctx = ObservedContext {
                    parsed: (!case["parsed"].is_null()).then(|| date(&case["parsed"])),
                    suffix: case["suffix"].as_bool(),
                    clock: date(&case["clock"]),
                    offset_minutes: case["offset_minutes"].as_i64().unwrap(),
                    calls: Vec::new(),
                    unavailable: false,
                };
                let result = item_argument::resolve(input.as_deref(), &mut ctx);
                let actual = match result {
                    Ok(value) => serde_json::json!({"value":value,"error":null,"calls":ctx.calls}),
                    Err(ArgumentError::DateRange) => {
                        serde_json::json!({"value":null,"error":"DateRange","calls":ctx.calls})
                    }
                    Err(other) => panic!("unexpected {other:?}"),
                };
                let expected = serde_json::json!({"value":case["value"],"error":case["error"],"calls":case["calls"]});
                assert_eq!(
                    actual, expected,
                    "arch={} culture={} offset={} case={index}",
                    arch["architecture"], profile["culture"], profile["offset_minutes"]
                );
            }
        }
    }
}
#[test]
fn unavailable_date_profile_is_an_error_before_suffix_or_clock() {
    let mut ctx = context();
    ctx.unavailable = true;
    let result = item_argument::resolve(Some(&"1sec".encode_utf16().collect::<Vec<_>>()), &mut ctx);
    assert_eq!(
        (result, ctx.calls),
        (Err(ArgumentError::ProfileUnavailable), vec!["date"])
    );
}
#[test]
fn queue_argument_bound_is_checked_before_context_calls() {
    let mut ctx = context();
    let result = item_argument::resolve(Some(&[b'1' as u16; 513]), &mut ctx);
    assert_eq!((result, ctx.calls), (Err(ArgumentError::Bound), vec![]));
}
#[test]
fn invalid_clock_kind_is_rejected_before_zone_conversion() {
    let mut ctx = context();
    ctx.clock = ArgumentDateTime::new(ctx.clock.ticks(), DateTimeKind::Utc).unwrap();
    let result = item_argument::resolve(Some(&"1sec".encode_utf16().collect::<Vec<_>>()), &mut ctx);
    assert_eq!(
        (result, ctx.calls),
        (
            Err(ArgumentError::ProfileContract),
            vec!["date", "suffix", "clock"]
        )
    );
}
#[test]
fn parser_resolves_each_relative_attribute_with_its_own_local_clock_sample() {
    let input = LegacyBytes::<1024, true>::from_bytes(
        "sword[A:v(1sec),B:v(2sec)]"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let mut ctx = context();
    let parsed = item_class_ex::parse(Some(&input), |raw| {
        item_argument::resolve(Some(raw), &mut ctx).map_err(Into::into)
    })
    .unwrap();
    assert_eq!(
        (
            parsed.attributes.iter().map(|a| a.arg).collect::<Vec<_>>(),
            ctx.calls
        ),
        (
            vec![1, 2],
            vec![
                "date", "suffix", "clock", "utc", "date", "suffix", "clock", "utc"
            ]
        )
    );
}

#[test]
fn invalid_clr_date_ticks_are_rejected_without_coercion() {
    assert_eq!(
        [-1, 3_155_378_976_000_000_000]
            .map(|ticks| ArgumentDateTime::new(ticks, DateTimeKind::Unspecified)),
        [Err(ArgumentError::DateRange), Err(ArgumentError::DateRange)]
    );
}
#[test]
fn inconsistent_zone_result_is_rejected_instead_of_reinterpreted_as_utc() {
    struct BadZone;
    impl ArgumentContext for BadZone {
        fn parse_date(&mut self, _: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError> {
            Err(ArgumentError::ProfileUnavailable)
        }
        fn ends_with_seconds(&mut self, _: &[u16]) -> Result<bool, ArgumentError> {
            Err(ArgumentError::ProfileUnavailable)
        }
        fn now_local(&mut self) -> Result<ArgumentDateTime, ArgumentError> {
            Err(ArgumentError::ProfileUnavailable)
        }
        fn local_to_utc(
            &mut self,
            local: ArgumentDateTime,
        ) -> Result<ArgumentDateTime, ArgumentError> {
            Ok(local)
        }
    }
    assert_eq!(
        item_argument::date_to_argument(
            ArgumentDateTime::new(621_355_968_000_000_000, DateTimeKind::Local).unwrap(),
            &mut BadZone
        ),
        Err(ArgumentError::ProfileContract)
    );
}
#[test]
fn numeric_argument_does_not_require_an_available_temporal_profile() {
    let mut ctx = context();
    ctx.unavailable = true;
    let result = item_argument::resolve(
        Some(&"-2147483648".encode_utf16().collect::<Vec<_>>()),
        &mut ctx,
    );
    assert_eq!((result, ctx.calls), (Ok(i32::MIN), vec![]));
}
