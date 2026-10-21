use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    shop::{BUY_FAILED, BuyPlan, BuyResult, ShopRow, Shops, plan},
    shop_price::Arch,
};

fn row(order: i16, item: &str, price_class: &str, price: i32) -> ShopRow {
    ShopRow {
        shop_id: "FERGHUS_Shop".to_owned(),
        order,
        item_class: item.to_owned(),
        count: 2,
        price_item_class: price_class.to_owned(),
        price_count: price,
        character_bind: false,
        restriction_count: 0,
        feature: None,
    }
}

#[test]
fn shop_rows_resolve_prices_and_plan_buys_like_the_processor() {
    let mut restricted = row(4, "potion", "gold", 10);
    restricted.restriction_count = 3;
    let mut disabled = row(5, "potion", "gold", 10);
    disabled.feature = Some("Disabled_Feature".to_owned());
    let mut duplicate = row(1, "other", "gold", 999);
    duplicate.shop_id = "ferghus_shop".to_owned();
    let shops = Shops::load(
        vec![
            row(1, "potion", "gold", 1234567),
            row(2, "potion", "stamp", 1234567),
            row(3, "sword[ENHANCE:3]", "gold", 10),
            restricted,
            disabled,
            row(6, "potion", "ap", 10),
            duplicate,
        ],
        1.0,
        Arch::X64,
    )
    .unwrap();
    // Ceiling10 for gold, Ceiling (Single rounding on x64) for other price items.
    assert_eq!(shops.get("ferghus_SHOP", 1).unwrap().price_count, 1234570);
    assert_eq!(shops.get("FERGHUS_Shop", 2).unwrap().price_count, 1234568);
    // The first duplicate (ShopID.ToLower(), Order) is kept.
    assert_eq!(shops.get("FERGHUS_Shop", 1).unwrap().item_class, "potion");
    assert_eq!(shops.rows_of("Ferghus_Shop").count(), 6);
    let policy = FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), Some("3.09.00".to_owned())),
        ("Disabled_Feature".to_owned(), None),
    ])
    .unwrap();
    let known = |c: &str| c == "potion";
    assert_eq!(
        plan(&shops, &policy, "FERGHUS_Shop", 1, 0, known),
        BuyPlan::Silent
    );
    assert_eq!(
        plan(&shops, &policy, "NOBODY_Shop", 1, 1, known),
        BuyPlan::Silent
    );
    let BuyPlan::Trade {
        give_count, price, ..
    } = plan(&shops, &policy, "FERGHUS_Shop", 1, 3, known)
    else {
        panic!("trade expected")
    };
    assert_eq!((give_count, price), (6, 1234570 * 3));
    let BuyPlan::Refused(result) = plan(&shops, &policy, "FERGHUS_Shop", 5, 1, known) else {
        panic!("refusal expected")
    };
    assert_eq!(
        result,
        BuyResult::failed(shops.get("FERGHUS_Shop", 5).unwrap(), BUY_FAILED)
    );
    assert_eq!(
        (
            result.item_count,
            result.price_count,
            result.restriction_order
        ),
        (2, 10, -1)
    );
    for order in [3, 4, 6] {
        assert_eq!(
            plan(&shops, &policy, "FERGHUS_Shop", order, 1, known),
            BuyPlan::Unsupported
        );
    }
    assert!(matches!(
        plan(&shops, &policy, "FERGHUS_Shop", 1, 1, |_| false),
        BuyPlan::Refused(_)
    ));
    assert_eq!(
        plan(&shops, &policy, "Buyback", 0, 1, known),
        BuyPlan::Unsupported
    );
}
