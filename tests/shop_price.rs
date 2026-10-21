use serde_json::Value;
use vindictus_server::domain::shop_price::{Arch, ceiling, ceiling10, ratio};

#[test]
fn price_calc_matches_the_recorded_framework_results_per_architecture() {
    let fixture: Value = serde_json::from_str(
        include_str!("fixtures/framework-shop-price.json").trim_start_matches('\u{feff}'),
    )
    .unwrap();
    let mut checked = 0;
    for (arch, key) in [(Arch::X64, "x64"), (Arch::X86, "x86")] {
        for case in fixture[key].as_array().unwrap() {
            let r = ratio(case["ratio"].as_str().unwrap()).unwrap();
            let price = case["price"].as_i64().unwrap() as i32;
            let (Some(c), Some(c10)) = (ceiling(price, r, arch), ceiling10(price, r, arch)) else {
                assert_eq!(arch, Arch::X86);
                assert_ne!(r, 1.0);
                continue;
            };
            assert_eq!(
                i64::from(c),
                case["ceiling"].as_i64().unwrap(),
                "{key} {case}"
            );
            assert_eq!(
                i64::from(c10),
                case["ceiling10"].as_i64().unwrap(),
                "{key} {case}"
            );
            checked += 1;
        }
    }
    // x64: every case; x86: ratios "1" and "0" (both 1), 36 prices each.
    assert_eq!(checked, 216 + 72);
}
