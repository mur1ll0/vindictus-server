use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    marketplace::{self as wire, DetailOption, Kind, Message, TradeItemInfo},
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
        "trade_category_search" => Kind::TradeCategorySearch,
        "trade_item_class_list_search" => Kind::TradeItemClassListSearch,
        "trade_cancel_my_item" => Kind::TradeCancelMyItem,
        "trade_cancel_my_item_result" => Kind::TradeCancelMyItemResult,
        "trade_request_my_item" => Kind::TradeRequestMyItem,
        "trade_my_item_infos" => Kind::TradeMyItemInfos,
        "trade_purchase_item" => Kind::TradePurchaseItem,
        "trade_purchase_item_result" => Kind::TradePurchaseItemResult,
        "trade_search_result" => Kind::TradeSearchResult,
        "insert_trade_order" => Kind::InsertTradeOrder,
        "insert_trade_order_result" => Kind::InsertTradeOrderResult,
        "insert_trade_tir_order" => Kind::InsertTradeTirOrder,
        "start_trade_session" => Kind::StartTradeSession,
        "end_trade_session" => Kind::EndTradeSession,
        _ => panic!("unknown fixture type"),
    }
}
fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn options(ours: &Option<Vec<Option<DetailOption>>>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(list) = ours else { return };
    let theirs = theirs.as_array().unwrap();
    assert_eq!(list.len(), theirs.len(), "case {n}");
    for (o, t) in list.iter().zip(theirs) {
        assert_eq!(o.is_none(), t.is_null(), "case {n}");
        if let Some(o) = o {
            assert_eq!(o.key.as_deref(), t["Key"].as_str(), "case {n}");
            assert_eq!(i64::from(o.value), int(&t["Value"]), "case {n}");
            assert_eq!(i64::from(o.search_type), int(&t["SearchType"]), "case {n}");
        }
    }
}
fn items(ours: &Option<Vec<Option<TradeItemInfo>>>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(list) = ours else { return };
    let theirs = theirs.as_array().unwrap();
    assert_eq!(list.len(), theirs.len(), "case {n}");
    for (o, t) in list.iter().zip(theirs) {
        assert_eq!(o.is_none(), t.is_null(), "case {n}");
        let Some(i) = o else { continue };
        for (ours, key) in [
            (i.tid, "TID"),
            (i.cid, "CID"),
            (i.item_count, "ItemCount"),
            (i.item_price, "ItemPrice"),
            (i64::from(i.close_date), "CloseDate"),
            (i64::from(i.max_armor_condition), "MaxArmorCondition"),
            (i64::from(i.color1), "color1"),
            (i64::from(i.color2), "color2"),
            (i64::from(i.color3), "color3"),
            (i64::from(i.min_price), "MinPrice"),
            (i64::from(i.max_price), "MaxPrice"),
            (i64::from(i.avg_price), "AvgPrice"),
            (i64::from(i.trade_type), "TradeType"),
        ] {
            assert_eq!(ours, int(&t[key]), "case {n} {key}");
        }
        assert_eq!(
            i.character_name.as_deref(),
            t["ChracterName"].as_str(),
            "case {n}"
        );
        assert_eq!(i.item_class.as_deref(), t["ItemClass"].as_str(), "case {n}");
        assert_eq!(
            i.attribute_ex.as_deref(),
            t["AttributeEX"].as_str(),
            "case {n}"
        );
        assert_eq!(
            i.attribute_ex.is_some(),
            t["HasAttribute"].as_bool().unwrap(),
            "case {n}"
        );
    }
}

fn check(case: &Value, n: usize) {
    let kind = kind(case["kind"].as_str().unwrap());
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let category = case["category"].as_i64().unwrap() as i32;
    let ingress = wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap();
    let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(ingress, decoded, "case {n}");
    assert_eq!(
        &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
        typed.as_slice(),
        "case {n}"
    );
    assert_eq!(decoded.is_none(), case["is_null"].as_bool().unwrap());
    let f = &case["fields"];
    match &decoded {
        None => assert!(f.is_null()),
        Some(Message::TradeRequestMyItem)
        | Some(Message::StartTradeSession)
        | Some(Message::EndTradeSession) => {}
        Some(Message::TradeCategorySearch(m)) => {
            assert_eq!(m.trade_category.as_deref(), f["tradeCategory"].as_str());
            assert_eq!(
                m.trade_category_sub.as_deref(),
                f["tradeCategorySub"].as_str()
            );
            for (ours, key) in [
                (m.min_level, "minLevel"),
                (m.max_level, "maxLevel"),
                (m.unique_number, "uniqueNumber"),
                (m.chunk_page_number, "ChunkPageNumber"),
                (m.order, "Order"),
            ] {
                assert_eq!(i64::from(ours), int(&f[key]), "case {n} {key}");
            }
            assert_eq!(m.is_descending, f["isDescending"].as_bool().unwrap());
            options(&m.detail_options, &f["DetailOptions"], n);
        }
        Some(Message::TradeItemClassListSearch(m)) => {
            let theirs = &f["ItemClassList"];
            assert_eq!(m.item_class_list.is_none(), theirs.is_null(), "case {n}");
            if let Some(list) = &m.item_class_list {
                let theirs: Vec<Option<&str>> = theirs
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(Value::as_str)
                    .collect();
                let ours: Vec<Option<&str>> = list.iter().map(Option::as_deref).collect();
                assert_eq!(ours, theirs, "case {n}");
            }
            for (ours, key) in [
                (m.unique_number, "uniqueNumber"),
                (m.chunk_page_number, "ChunkPageNumber"),
                (m.order, "Order"),
            ] {
                assert_eq!(i64::from(ours), int(&f[key]), "case {n} {key}");
            }
            assert_eq!(m.is_descending, f["isDescending"].as_bool().unwrap());
            options(&m.detail_options, &f["DetailOptions"], n);
        }
        Some(Message::TradeCancelMyItem(m)) => {
            assert_eq!(m.tid, int(&f["TID"]), "case {n}");
            assert_eq!(i64::from(m.unique_number), int(&f["UniqueNumber"]));
        }
        Some(Message::TradeCancelMyItemResult(m)) => {
            assert_eq!(i64::from(m.unique_number), int(&f["UniqueNumber"]));
            assert_eq!(i64::from(m.result), int(&f["Result"]), "case {n}");
        }
        Some(Message::TradeMyItemInfos(m)) => {
            items(&m.trade_item_list, &f["TradeItemList"], n);
            assert_eq!(i64::from(m.result), int(&f["result"]), "case {n}");
        }
        Some(Message::TradePurchaseItem(m)) => {
            assert_eq!(m.tid, int(&f["TID"]), "case {n}");
            assert_eq!(i64::from(m.purchase_count), int(&f["PurchaseCount"]));
            assert_eq!(i64::from(m.unique_number), int(&f["UniqueNumber"]));
        }
        Some(Message::TradePurchaseItemResult(m)) => {
            assert_eq!(i64::from(m.unique_number), int(&f["UniqueNumber"]));
            assert_eq!(i64::from(m.result), int(&f["Result"]), "case {n}");
            assert_eq!(i64::from(m.left_number), int(&f["LeftNumber"]));
        }
        Some(Message::TradeSearchResult(m)) => {
            items(&m.trade_item_list, &f["TradeItemList"], n);
            assert_eq!(i64::from(m.unique_number), int(&f["UniqueNumber"]));
            assert_eq!(m.is_more_result, f["IsMoreResult"].as_bool().unwrap());
            assert_eq!(i64::from(m.result), int(&f["result"]), "case {n}");
        }
        Some(Message::InsertTradeOrder(m)) => {
            assert_eq!(m.item_id, int(&f["ItemID"]), "case {n}");
            assert_eq!(i64::from(m.num), int(&f["Num"]), "case {n}");
            assert_eq!(i64::from(m.duration_min), int(&f["DurationMin"]));
            assert_eq!(i64::from(m.unit_price), int(&f["UnitPrice"]));
            assert_eq!(i64::from(m.trade_type), int(&f["TradeType"]));
        }
        Some(Message::InsertTradeOrderResult(code)) => {
            assert_eq!(i64::from(*code), int(&f["Result"]), "case {n}")
        }
        Some(Message::InsertTradeTirOrder(m)) => {
            assert_eq!(i64::from(m.num), int(&f["Num"]), "case {n}");
            assert_eq!(i64::from(m.duration_min), int(&f["DurationMin"]));
            assert_eq!(i64::from(m.unit_price), int(&f["UnitPrice"]));
        }
    }
}

#[test]
fn framework_marketplace_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-marketplace-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    for (n, case) in cases.iter().enumerate() {
        check(case, n);
    }
    for kind in [
        Kind::TradeCategorySearch,
        Kind::TradeItemClassListSearch,
        Kind::TradeCancelMyItem,
        Kind::TradeCancelMyItemResult,
        Kind::TradeRequestMyItem,
        Kind::TradeMyItemInfos,
        Kind::TradePurchaseItem,
        Kind::TradePurchaseItemResult,
        Kind::TradeSearchResult,
        Kind::InsertTradeOrder,
        Kind::InsertTradeOrderResult,
        Kind::InsertTradeTirOrder,
        Kind::StartTradeSession,
        Kind::EndTradeSession,
    ] {
        let named = cases
            .iter()
            .filter(|c| self::kind(c["kind"].as_str().unwrap()) == kind)
            .count();
        assert!(named >= 2, "{kind:?} has no fixture");
        // The registered categories and GUIDs are the recovered framework type table.
        assert!(cases.iter().any(|c| {
            self::kind(c["kind"].as_str().unwrap()) == kind
                && c["category"].as_i64() == Some(i64::from(kind.category()))
        }));
    }
}

#[test]
fn marketplace_ingress_refuses_other_types_and_truncated_bodies() {
    let typed = [1u8, 0, 0, 0, 0, 0, 0, 0, 9, 0, 0, 0, 3, 0, 0, 0, 1];
    assert!(matches!(
        wire::decode_ingress(Kind::TradePurchaseItem, 310, &typed, DEFAULT_MAX_BODY),
        Err(wire::MarketplaceWireError::Type)
    ));
    assert!(wire::decode(Kind::TradePurchaseItem, &typed[..10], DEFAULT_MAX_BODY).is_err());
    assert!(
        wire::decode(
            Kind::TradePurchaseItem,
            &[typed.as_slice(), &[0]].concat(),
            DEFAULT_MAX_BODY
        )
        .is_err()
    );
    assert_eq!(
        wire::encode(
            Kind::TradePurchaseItemResult,
            Some(&Message::InsertTradeOrderResult(0)),
            DEFAULT_MAX_BODY
        ),
        Err(wire::MarketplaceWireError::Kind)
    );
}
