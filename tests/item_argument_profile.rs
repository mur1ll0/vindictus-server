use serde_json::{Value, json};
use vindictus_server::domain::{
    item_argument::{self, ArgumentContext, ArgumentDateTime, ArgumentError, DateTimeKind},
    item_argument_profile::{ReferenceVmArgumentContext, parse_reference_date},
    item_class_ex,
    legacy_user_data::LegacyBytes,
};
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/framework-item-argument-profile.json"
    ))
    .unwrap()
}
fn units(value: &Value) -> Option<Vec<u16>> {
    value.as_str().map(|hex| {
        (0..hex.len())
            .step_by(4)
            .map(|i| {
                u16::from_le_bytes([
                    u8::from_str_radix(&hex[i..i + 2], 16).unwrap(),
                    u8::from_str_radix(&hex[i + 2..i + 4], 16).unwrap(),
                ])
            })
            .collect()
    })
}
fn date(value: &Value) -> ArgumentDateTime {
    let kind = match value["kind"].as_str().unwrap() {
        "Unspecified" => DateTimeKind::Unspecified,
        "Utc" => DateTimeKind::Utc,
        "Local" => DateTimeKind::Local,
        _ => panic!("fixture kind"),
    };
    ArgumentDateTime::new(value["ticks"].as_str().unwrap().parse().unwrap(), kind).unwrap()
}
fn pack(date: ArgumentDateTime) -> Value {
    let kind = match date.kind() {
        DateTimeKind::Unspecified => "Unspecified",
        DateTimeKind::Utc => "Utc",
        DateTimeKind::Local => "Local",
    };
    json!({"ticks":date.ticks().to_string(),"kind":kind})
}
struct Recorded<C> {
    inner: C,
    calls: Vec<&'static str>,
}
impl<C: ArgumentContext> ArgumentContext for Recorded<C> {
    fn parse_date(&mut self, text: &[u16]) -> Result<Option<ArgumentDateTime>, ArgumentError> {
        self.calls.push("date");
        self.inner.parse_date(text)
    }
    fn ends_with_seconds(&mut self, text: &[u16]) -> Result<bool, ArgumentError> {
        self.calls.push("suffix");
        self.inner.ends_with_seconds(text)
    }
    fn now_local(&mut self) -> Result<ArgumentDateTime, ArgumentError> {
        self.calls.push("clock");
        self.inner.now_local()
    }
    fn local_to_utc(&mut self, local: ArgumentDateTime) -> Result<ArgumentDateTime, ArgumentError> {
        self.calls.push("utc");
        self.inner.local_to_utc(local)
    }
}
#[test]
fn real_reference_adapter_matches_framework_parse_results_decisions_and_lazy_calls() {
    for arch in fixture()["profiles"].as_array().unwrap() {
        for profile in arch["profiles"].as_array().unwrap() {
            let clock = date(&profile["utc_clock"]);
            for (index, case) in profile["cases"].as_array().unwrap().iter().enumerate() {
                let input = units(&case["input_hex"]);
                let mut ctx = Recorded {
                    inner: ReferenceVmArgumentContext::new(|| Ok(clock)),
                    calls: Vec::new(),
                };
                let result = item_argument::resolve(input.as_deref(), &mut ctx);
                let actual = match result {
                    Ok(value) => json!({"value":value,"error":null,"calls":ctx.calls}),
                    Err(ArgumentError::DateRange) => {
                        json!({"value":null,"error":"DateRange","calls":ctx.calls})
                    }
                    Err(error) => panic!(
                        "unexpected {error:?}: arch={} case={index}",
                        arch["architecture"]
                    ),
                };
                assert_eq!(
                    actual,
                    json!({"value":case["value"],"error":case["error"],"calls":case["calls"]}),
                    "arch={} clock={} case={index}",
                    arch["architecture"],
                    profile["utc_clock"]["ticks"]
                );
                if case["calls"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|c| c == "date")
                {
                    let actual = parse_reference_date(input.as_deref().unwrap())
                        .unwrap()
                        .map(pack);
                    assert_eq!(
                        actual,
                        (!case["parsed"].is_null()).then(|| case["parsed"].clone()),
                        "parsed arch={} case={index}",
                        arch["architecture"]
                    );
                }
            }
        }
    }
}
#[test]
fn unsupported_grammar_is_rejected_without_reading_the_clock() {
    for arch in fixture()["profiles"].as_array().unwrap() {
        for encoded in arch["unsupported_hex"].as_array().unwrap() {
            let input = units(encoded).unwrap();
            let mut calls = 0;
            let result = {
                let mut ctx = ReferenceVmArgumentContext::new(|| {
                    calls += 1;
                    Err(ArgumentError::ProfileUnavailable)
                });
                item_argument::resolve(Some(&input), &mut ctx)
            };
            assert_eq!((result, calls), (Err(ArgumentError::ProfileUnavailable), 0));
        }
    }
}
#[test]
fn relative_argument_propagates_clock_failure_without_invalid_zero_fallback() {
    let mut ctx = ReferenceVmArgumentContext::new(|| Err(ArgumentError::ProfileUnavailable));
    assert_eq!(
        item_argument::resolve(Some(&"1sec".encode_utf16().collect::<Vec<_>>()), &mut ctx),
        Err(ArgumentError::ProfileUnavailable)
    );
}
#[test]
fn reference_profile_rejects_a_non_utc_clock_sample() {
    let mut ctx =
        ReferenceVmArgumentContext::new(|| ArgumentDateTime::new(1, DateTimeKind::Unspecified));
    assert_eq!(
        item_argument::resolve(Some(&"1sec".encode_utf16().collect::<Vec<_>>()), &mut ctx),
        Err(ArgumentError::ProfileContract)
    );
}
#[test]
fn canonical_dates_and_relative_arguments_integrate_with_item_parser() {
    let text = "sword[A:v(1970-01-01),B:v(1sec),C:v(-1sec)]";
    let input = LegacyBytes::<1024, true>::from_bytes(
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    )
    .unwrap();
    let mut reads = 0;
    let parsed = {
        let mut ctx = ReferenceVmArgumentContext::new(|| {
            reads += 1;
            ArgumentDateTime::new(
                621_355_968_000_000_000 + (reads - 1) * 10_000_000,
                DateTimeKind::Utc,
            )
        });
        item_class_ex::parse(Some(&input), |raw| {
            item_argument::resolve(Some(raw), &mut ctx).map_err(Into::into)
        })
        .unwrap()
    };
    assert_eq!(
        (
            parsed.attributes.iter().map(|a| a.arg).collect::<Vec<_>>(),
            reads
        ),
        (vec![0, 1, 0], 2)
    );
}
#[test]
fn parser_bound_is_enforced_before_inspecting_unsupported_input() {
    assert_eq!(
        parse_reference_date(&[0xffff; 513]),
        Err(ArgumentError::Bound)
    );
}
#[test]
fn fixed_zone_conversion_rejects_a_non_local_date() {
    let mut ctx = ReferenceVmArgumentContext::new(|| Err(ArgumentError::ProfileUnavailable));
    assert_eq!(
        ctx.local_to_utc(ArgumentDateTime::new(1, DateTimeKind::Utc).unwrap()),
        Err(ArgumentError::ProfileContract)
    );
}

#[test]
fn original_attribute_grammar_discards_time_colons_before_calling_the_resolver() {
    let input = LegacyBytes::<1024, true>::from_bytes(
        "sword[A:v(1970-01-01T00:00:00)]"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect(),
    )
    .unwrap();
    let mut calls = 0;
    let parsed = item_class_ex::parse(Some(&input), |_| {
        calls += 1;
        Err(item_class_ex::ItemClassExError::ArgumentUnsupported)
    })
    .unwrap();
    assert_eq!((parsed.attributes.len(), calls), (0, 0));
}
