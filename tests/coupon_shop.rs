use serde_json::Value;
use vindictus_server::domain::{
    coupon_shop::{BuyPlan, CouponShops, CouponTables, restrict_set},
    feature_policy::FeaturePolicy,
};
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    coupon_shop::{self as wire, Kind, Message},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn kind(name: &str) -> Kind {
    match name {
        "coupon_shop_item_info_query" => Kind::ItemInfoQuery,
        "coupon_shop_item_info_result" => Kind::ItemInfoResult,
        "coupon_shop_item_buy" => Kind::ItemBuy,
        _ => panic!("unknown fixture type"),
    }
}
fn uuid_bytes(v: u128) -> [u8; 16] {
    v.to_be_bytes()
}
fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}

#[test]
fn framework_coupon_shop_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-coupon-shop-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    for (n, case) in cases.iter().enumerate() {
        let kind = kind(case["kind"].as_str().unwrap());
        let body = bytes(&case["body"]);
        let typed = bytes(&case["typed_body"]);
        let category = case["category"].as_i64().unwrap() as i32;
        // An unregistered type serializes with its GUID envelope even when typed.
        let object = uuid_bytes(0x81c5fe01_027c_3e1c_98d5_da9c9862aa21);
        if kind == Kind::ItemBuy && typed[..16] == object {
            // A null unregistered message is the System.Object envelope.
            assert_eq!((&typed[16..], case["is_null"].as_bool()), (&[0u8][..], Some(true)));
            continue;
        }
        let typed = if kind == Kind::ItemBuy {
            assert_eq!(&typed[..16], kind.type_id().as_bytes());
            typed[16..].to_vec()
        } else {
            typed
        };
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        // The unregistered buy message arrives only in the GUID envelope.
        if kind == Kind::ItemBuy || category != 0 {
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "case {n}"
            );
        }
        if kind == Kind::ItemBuy {
            assert_eq!(
                (category, wire::ingress_kind(category, &body)),
                (0, Some(kind))
            );
        }
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        assert_eq!(decoded.is_none(), case["is_null"].as_bool().unwrap());
        let f = &case["fields"];
        match &decoded {
            None => assert!(f.is_null()),
            Some(Message::ItemInfoQuery(v)) => assert_eq!(i64::from(*v), int(&f["ShopVersion"])),
            Some(Message::ItemBuy(m)) => {
                assert_eq!(i64::from(m.shop_id), int(&f["ShopID"]));
                assert_eq!(i64::from(m.order), int(&f["Order"]));
                assert_eq!(i64::from(m.count), int(&f["Count"]));
            }
            Some(Message::ItemInfoResult(m)) => {
                assert_eq!(i64::from(m.shop_version), int(&f["ShopVersion"]));
                let theirs = &f["RestrictionCountDic"];
                assert_eq!(m.restrictions.is_none(), theirs.is_null(), "case {n}");
                let rows = m.restrictions.as_deref().unwrap_or_default();
                let theirs = theirs.as_array().map(Vec::as_slice).unwrap_or_default();
                assert_eq!(rows.len(), theirs.len(), "case {n}");
                for ((order, r), t) in rows.iter().zip(theirs) {
                    assert_eq!(i64::from(*order), int(&t["key"]));
                    let r = r.unwrap();
                    assert_eq!(i64::from(r.buyable_count), int(&t["value"]["BuyableCount"]));
                    assert_eq!(
                        r.next_reset_ticks_diff,
                        int(&t["value"]["NextResetTicksDiff"])
                    );
                }
            }
        }
    }
    for kind in [Kind::ItemInfoQuery, Kind::ItemInfoResult, Kind::ItemBuy] {
        assert!(
            cases
                .iter()
                .any(|c| c["kind"].as_str().map(self::kind) == Some(kind))
        );
    }
}

#[test]
fn coupon_shop_ingress_refuses_other_types() {
    // The query's GUID envelope is accepted, another GUID is not.
    let mut query = Kind::ItemInfoQuery.type_id().as_bytes().to_vec();
    query.extend([1, 0, 40]);
    assert_eq!(wire::ingress_kind(0, &query), Some(Kind::ItemInfoQuery));
    assert_eq!(
        wire::decode_ingress(Kind::ItemInfoQuery, 0, &query, DEFAULT_MAX_BODY).unwrap(),
        Some(Message::ItemInfoQuery(40))
    );
    let mut other = query.clone();
    other[0] ^= 0xff;
    assert_eq!(wire::ingress_kind(0, &other), None);
    // A category-0 buy body is never taken by its category alone.
    assert_eq!(wire::ingress_kind(0, &[1, 0, 40, 0, 4, 0, 0, 0, 1]), None);
    assert_eq!(
        wire::ingress_kind(165, &[1, 0, 40]),
        Some(Kind::ItemInfoQuery)
    );
    assert!(matches!(
        wire::decode_ingress(Kind::ItemBuy, 165, &[1, 0, 40], DEFAULT_MAX_BODY),
        Err(wire::CouponShopWireError::Type)
    ));
}

fn policy() -> FeaturePolicy {
    FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
        ("shop_on".to_owned(), Some("1".to_owned())),
        ("Off".to_owned(), None),
    ])
    .unwrap()
}
fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}
/// 2014-01-01 00:00 UTC in .NET ticks.
const RESET: i64 = 635_241_312_000_000_000;
const DAY: i64 = 864_000_000_000;

fn shops() -> CouponShops {
    CouponShops::load(CouponTables {
        versions: vec![
            (
                40,
                s("2015.07.15 ~ 2015.09.09"),
                s("ticket_allusergoal_2015"),
                s("shop_on"),
            ),
            (41, None, s("ticket"), s("Off")),
        ],
        items: vec![
            (
                40,
                1,
                s("pattern_regina_platinum"),
                1,
                40,
                true,
                false,
                None,
                Some(0),
                Some(1),
                None,
            ),
            (
                40,
                2,
                s("avatar_cook_female_nostat_head"),
                1,
                20,
                false,
                false,
                None,
                None,
                None,
                None,
            ),
            (
                40,
                3,
                s("weekly_box"),
                2,
                5,
                false,
                true,
                Some(RESET),
                Some(7),
                Some(3),
                None,
            ),
            (
                40,
                4,
                s("hidden"),
                1,
                1,
                false,
                false,
                None,
                None,
                None,
                s("Off"),
            ),
            (41, 1, s("x"), 1, 1, false, false, None, None, None, None),
        ],
    })
    .unwrap()
}

#[test]
fn coupon_purchases_follow_the_buy_processor() {
    let shops = shops();
    let p = policy();
    let ok = |_: &str| Some(true);
    let exists = |c: &str| c != "unknown";
    assert_eq!(shops.plan(&p, 40, 2, 0, ok, exists), BuyPlan::Silent);
    assert_eq!(shops.plan(&p, 99, 1, 1, ok, exists), BuyPlan::Silent);
    // The version's Feature, then the item's.
    assert_eq!(shops.plan(&p, 41, 1, 1, ok, exists), BuyPlan::Silent);
    assert_eq!(shops.plan(&p, 40, 4, 1, ok, exists), BuyPlan::Silent);
    assert_eq!(shops.plan(&p, 40, 9, 1, ok, exists), BuyPlan::Silent);
    // IsAvailableFor fails with the row's unit values.
    let BuyPlan::Refused(r) = shops.plan(&p, 40, 2, 3, |_| Some(false), exists) else {
        panic!("refused");
    };
    assert_eq!(
        (
            r.result,
            r.item_count,
            r.price_item_class.as_str(),
            r.price_count,
            r.restriction_count
        ),
        (2, 1, "ticket_allusergoal_2015", 20, -1)
    );
    // Three avatars: 3 given for 60 tickets.
    let BuyPlan::Trade {
        give, price, item, ..
    } = shops.plan(&p, 40, 2, 3, |_| None, exists)
    else {
        panic!("trade");
    };
    assert_eq!((give, price, item.shop_item_id()), (3, 60, (40 << 16) | 2));
}

#[test]
fn buy_restrictions_follow_the_shop_session_procedures() {
    let shops = shops();
    let items = shops.items(40).unwrap();
    let (once, weekly) = (&items[0], &items[2]);
    assert!(once.is_restricted() && !once.has_restrict_time());
    assert!(weekly.has_restrict_time());
    let now = RESET + 10 * DAY + 3_600 * 10_000_000;
    // The weekly period started on day 7.
    assert_eq!(weekly.period_start(now), Some(RESET + 7 * DAY));
    // Never bought: the whole count, no reset.
    assert_eq!(once.restriction(None, now), (1, -1));
    assert_eq!(once.restriction(Some((RESET, 1)), now), (0, -1));
    // Bought twice this week: one left, reset at day 14 counted from the purchase.
    let bought = RESET + 8 * DAY;
    assert_eq!(weekly.restriction(Some((bought, 2)), now), (1, 6 * DAY));
    // A purchase of an earlier period does not count.
    assert_eq!(weekly.restriction(Some((RESET + DAY, 3)), now), (3, -1));
    // ShopBuyRestrictSet: above the maximum, a new row, an accumulation, a new period.
    assert_eq!(restrict_set(None, None, 2, 1, now), None);
    assert_eq!(restrict_set(None, None, 1, 1, now), Some(((now, 1), 1)));
    assert_eq!(restrict_set(Some((bought, 1)), None, 1, 1, now), None);
    assert_eq!(
        restrict_set(Some((bought, 2)), Some(RESET + 7 * DAY), 1, 3, now),
        Some(((now, 3), 3))
    );
    assert_eq!(
        restrict_set(Some((RESET + DAY, 3)), Some(RESET + 7 * DAY), 2, 3, now),
        Some(((now, 2), 2))
    );
    // The result message carries the count left of a restricted purchase.
    let version = shops.version(40).unwrap();
    let ok = CouponShops::succeeded(version, weekly, 4, 10, 3);
    assert_eq!(
        (ok.result, ok.restriction_order, ok.restriction_count),
        (1, 3, 0)
    );
    let free = CouponShops::succeeded(version, &items[1], 1, 20, -1);
    assert_eq!(free.restriction_count, -1);
}
