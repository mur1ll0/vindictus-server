use super::{cleanup_game_pool, isolated_game_pool, minimum_character};
use vindictus_server::{
    domain::marketplace::SearchOverhead,
    infrastructure::{minimum_give, minimum_mail, minimum_marketplace as market},
    protocol::{
        DEFAULT_MAX_BODY,
        marketplace::{self as wire, DetailOption, Kind, Message},
    },
};

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn decode(out: &market::Outcome, kind: Kind) -> Message {
    let p = out
        .packets
        .iter()
        .find(|p| p.category == kind.category())
        .unwrap_or_else(|| panic!("{kind:?} missing"));
    wire::decode(kind, &p.body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
}
fn order_code(out: &market::Outcome) -> i32 {
    match decode(out, Kind::InsertTradeOrderResult) {
        Message::InsertTradeOrderResult(code) => code,
        _ => unreachable!(),
    }
}
fn search(out: &market::Outcome) -> wire::TradeSearchResult {
    match decode(out, Kind::TradeSearchResult) {
        Message::TradeSearchResult(r) => r,
        _ => unreachable!(),
    }
}
async fn count(p: &sqlx::PgPool, cid: i64, class: &str) -> i64 {
    sqlx::query_scalar("SELECT COALESCE(sum(count),0)::bigint FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2")
        .bind(cid)
        .bind(utf16(class))
        .fetch_one(p)
        .await
        .unwrap()
}
async fn ore_id(p: &sqlx::PgPool, cid: i64) -> i64 {
    sqlx::query_scalar("SELECT id FROM game_items WHERE owner_id=$1 AND item_class_utf16=$2 ORDER BY count DESC LIMIT 1")
        .bind(cid)
        .bind(utf16("synthetic_ore"))
        .fetch_one(p)
        .await
        .unwrap()
}
fn order(
    item_id: i64,
    num: i32,
    price: i32,
    minutes: i32,
    trade_type: u8,
) -> wire::InsertTradeOrder {
    wire::InsertTradeOrder {
        item_id,
        num,
        duration_min: minutes,
        unit_price: price,
        trade_type,
    }
}
fn category(options: Option<Vec<Option<DetailOption>>>) -> wire::TradeCategorySearch {
    wire::TradeCategorySearch {
        trade_category: Some("material".into()),
        trade_category_sub: Some("MATERIAL_ORE ".into()),
        min_level: 0,
        max_level: 90,
        unique_number: 41,
        chunk_page_number: 1,
        order: 2,
        is_descending: false,
        detail_options: options,
    }
}
fn ids(base: i64) -> Vec<i64> {
    (0..64).map(|n| base + n).collect()
}
/// A second character "Evie" of the same account (the gold of an account is shared by
/// its characters, so purchases are made by the seller itself: the source allows it).
async fn other(p: &sqlx::PgPool) {
    sqlx::raw_sql(
        "INSERT INTO game_characters SELECT (jsonb_populate_record(NULL::game_characters,to_jsonb(c)||'{\"id\":2}')).* FROM game_characters c WHERE id=1;
         INSERT INTO game_character_info SELECT (jsonb_populate_record(NULL::game_character_info,to_jsonb(c)||'{\"id\":2,\"character_sn\":7}')).* FROM game_character_info c WHERE id=1;",
    )
    .execute(p)
    .await
    .unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1 WHERE id=2")
        .bind(utf16("Evie"))
        .execute(p)
        .await
        .unwrap();
}
async fn last_mail(p: &sqlx::PgPool, cid: i64) -> (i16, Vec<u8>, Vec<u8>, i32) {
    sqlx::query_as("SELECT m.mail_type,m.from_name_utf16,m.title_utf16,i.item_count FROM game_mail m JOIN game_mail_items i ON i.mail_id=m.mail_id WHERE m.to_cid=$1 ORDER BY m.mail_id DESC LIMIT 1")
        .bind(cid)
        .fetch_one(p)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn marketplace_registers_searches_buys_cancels_expires_and_mails() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    sqlx::query("UPDATE game_character_info SET name_utf16=$1,level=20 WHERE id=1")
        .bind(utf16("Fiona"))
        .execute(&p)
        .await
        .unwrap();
    other(&p).await;
    let assets = super::minimum_give::marketplace_assets();
    let owner = super::initial_inventory::key();
    for (cid, class, n) in [(1, "synthetic_gold", 10_000), (1, "synthetic_ore", 50)] {
        assert_eq!(
            minimum_give::lab_grant(&p, &assets, Some(cid), class, n, 9)
                .await
                .unwrap(),
            Ok(1)
        );
    }
    let ore = ore_id(&p, 1).await;
    // TradeInsertOrderProcessor refusals, none of which writes anything.
    for (m, code) in [
        (order(ore, 5, 0, 4320, 0), 2),
        (order(ore, 0, 100, 4320, 0), 2),
        (order(-7, 1, 100, 4320, 0), 2),
        (order(ore, 5, 701, 4320, 0), 1),
        (order(ore, 5, 100, 4320, 1), 11),
        (order(ore, 60, 100, 4320, 0), 2),
        (order(ore, 5, 50_000_000, 4320, 0), 1),
    ] {
        let out = market::insert_order(&p, &owner, &assets, 1, &m)
            .await
            .unwrap();
        assert_eq!(order_code(&out), code, "{m:?}");
    }
    assert_eq!(count(&p, 1, "synthetic_ore").await, 50);
    assert_eq!(count(&p, 1, "synthetic_gold").await, 10_000);
    // 30 ore at 100 for three days: fee 300/10000 * 100 * 30 * 3 = 270 gold.
    let out = market::insert_order(&p, &owner, &assets, 1, &order(ore, 30, 100, 4320, 0))
        .await
        .unwrap();
    assert_eq!(order_code(&out), 0);
    assert!(out.packets.len() == 2, "inventory sync and the result");
    assert_eq!(count(&p, 1, "synthetic_ore").await, 20);
    assert_eq!(count(&p, 1, "synthetic_gold").await, 9_730);
    let (tid, name, listed, attribute_ex): (i64, String, i32, Option<String>) = sqlx::query_as(
        "SELECT tid,character_name,item_count,attribute_ex FROM game_trade_items WHERE cid=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(
        (name.as_str(), listed, attribute_ex.as_deref()),
        ("Fiona", 30, Some(""))
    );
    let attrs: Vec<(String, i32)> =
        sqlx::query_as("SELECT type,value FROM game_trade_item_attributes WHERE tid=$1")
            .bind(tid)
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(attrs, [("PRICE".to_owned(), 100)]);
    // The seller's open registrations.
    let out = market::my_items(&p, &assets, 1).await.unwrap();
    let Message::TradeMyItemInfos(mine) = decode(&out, Kind::TradeMyItemInfos) else {
        unreachable!()
    };
    let list = mine.trade_item_list.unwrap();
    assert_eq!((mine.result, list.len()), (0, 1));
    let info = list[0].clone().unwrap();
    assert_eq!((info.tid, info.item_count, info.item_price), (tid, 30, 100));
    assert!(
        (259_190..=259_200).contains(&info.close_date),
        "{}",
        info.close_date
    );
    let out = market::my_items(&p, &assets, 2).await.unwrap();
    let Message::TradeMyItemInfos(none) = decode(&out, Kind::TradeMyItemInfos) else {
        unreachable!()
    };
    assert_eq!((none.result, none.trade_item_list), (7, None));
    // Searches: category (case and trailing blanks ignored), price option, class list.
    let mut overhead = SearchOverhead::default();
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &category(Some(Vec::new())))
            .await
            .unwrap(),
    );
    assert_eq!(
        (found.result, found.unique_number, found.is_more_result),
        (0, 41, false)
    );
    assert_eq!(found.trade_item_list.unwrap().len(), 1);
    let under = |v| {
        Some(vec![Some(DetailOption {
            key: Some("PRICE".into()),
            value: v,
            search_type: 1,
        })])
    };
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &category(under(150)))
            .await
            .unwrap(),
    );
    let first = found.trade_item_list.unwrap()[0].clone().unwrap();
    assert_eq!((first.tid, first.attribute_ex.as_deref()), (tid, Some("")));
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &category(under(50)))
            .await
            .unwrap(),
    );
    assert_eq!((found.result, found.trade_item_list), (7, None));
    let equal = Some(vec![Some(DetailOption {
        key: Some("price".into()),
        value: 100,
        search_type: 2,
    })]);
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &category(equal))
            .await
            .unwrap(),
    );
    assert_eq!(found.result, 0);
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &category(None))
            .await
            .unwrap(),
    );
    assert_eq!(found.result, 1);
    let mut other = category(Some(Vec::new()));
    other.trade_category = Some("WEAPON".into());
    let found = search(
        &market::category_search(&p, &assets, &mut overhead, &other)
            .await
            .unwrap(),
    );
    assert_eq!(found.result, 7);
    let classes = |list: Vec<Option<String>>| wire::TradeItemClassListSearch {
        item_class_list: Some(list),
        unique_number: 5,
        chunk_page_number: 1,
        order: 1,
        is_descending: true,
        detail_options: Some(Vec::new()),
    };
    let found = search(
        &market::class_search(
            &p,
            &assets,
            &mut overhead,
            2,
            &classes(vec![Some("synthetic_ore".into())]),
        )
        .await
        .unwrap(),
    );
    assert_eq!(
        (found.result, found.trade_item_list.map(|l| l.len())),
        (0, Some(1))
    );
    let found = search(
        &market::class_search(
            &p,
            &assets,
            &mut overhead,
            2,
            &classes(vec![Some("nope".into())]),
        )
        .await
        .unwrap(),
    );
    assert_eq!(found.result, 1);
    // Purchases (by the seller, as the source permits): 10 of 30 for 1000 gold, then
    // more than left, then a registration the gold does not cover.
    let buy = |tid, count| wire::TradePurchaseItem {
        tid,
        purchase_count: count,
        unique_number: 9,
    };
    let bought = |out: &market::Outcome| match decode(out, Kind::TradePurchaseItemResult) {
        Message::TradePurchaseItemResult(r) => (r.result, r.left_number, r.unique_number),
        _ => unreachable!(),
    };
    let out = market::purchase(&p, &owner, &assets, 1, &buy(tid, 10), ids(9000))
        .await
        .unwrap();
    assert_eq!(bought(&out), (0, 20, 9));
    assert_eq!(count(&p, 1, "synthetic_ore").await, 30);
    assert_eq!(count(&p, 1, "synthetic_gold").await, 8_730);
    // CheckCharacterBind: a bought non-equipment item is character bound.
    let bound: i64 = sqlx::query_scalar("SELECT COALESCE(sum(count),0)::bigint FROM game_items WHERE owner_id=1 AND item_class_utf16=$1 AND NOT tradable")
        .bind(utf16("synthetic_ore"))
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(bound, 10);
    let out = market::purchase(&p, &owner, &assets, 1, &buy(tid, 25), ids(9100))
        .await
        .unwrap();
    assert_eq!(bought(&out), (4, 20, 9));
    let ore = ore_id(&p, 1).await;
    let out = market::insert_order(&p, &owner, &assets, 1, &order(ore, 15, 700, 60, 0))
        .await
        .unwrap();
    assert_eq!(order_code(&out), 0);
    let dear: i64 = sqlx::query_scalar("SELECT tid FROM game_trade_items WHERE item_price=700")
        .fetch_one(&p)
        .await
        .unwrap();
    let out = market::purchase(&p, &owner, &assets, 1, &buy(dear, 15), ids(9200))
        .await
        .unwrap();
    assert_eq!(bought(&out).0, 2);
    assert_eq!(count(&p, 1, "synthetic_gold").await, 8_730);
    let out = market::cancel(
        &p,
        &owner,
        &assets,
        1,
        &wire::TradeCancelMyItem {
            tid: dear,
            unique_number: 1,
        },
        ids(9250),
    )
    .await
    .unwrap();
    assert!(
        matches!(decode(&out, Kind::TradeCancelMyItemResult), Message::TradeCancelMyItemResult(r) if r.result == 0)
    );
    let left: i32 = sqlx::query_scalar("SELECT item_count FROM game_trade_items WHERE tid=$1")
        .bind(tid)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(left, 20);
    let gold: Vec<(i64, i32)> = sqlx::query_as("SELECT cid,gold FROM game_trade_gold")
        .fetch_all(&p)
        .await
        .unwrap();
    assert_eq!(gold, [(1, 1000)]);
    // The seller's next GetMyThings mails the 1000 gold as AuctionItemMail.
    let out = market::get_my_things(&p, &owner, &assets, 1, ids(9300))
        .await
        .unwrap();
    assert_eq!(out.packets.len(), 1, "the Sold console count");
    let (kind, sender, title, n) = last_mail(&p, 1).await;
    assert_eq!((kind, n), (7, 1000));
    assert_eq!(sender, utf16("Heroes_GameUI_MailSender_AUCTION"));
    assert_eq!(
        title,
        utf16("#GameUI_Heroes_TradeMail_Sold_Title,synthetic_ore,10")
    );
    assert!(minimum_mail::take_notice(1));
    let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM game_trade_gold")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(pending, 0);
    // Only the seller cancels; the rest returns to the seller's inventory.
    let cancel = wire::TradeCancelMyItem {
        tid,
        unique_number: 3,
    };
    let canceled = |out: &market::Outcome| match decode(out, Kind::TradeCancelMyItemResult) {
        Message::TradeCancelMyItemResult(r) => (r.result, r.unique_number),
        _ => unreachable!(),
    };
    let out = market::cancel(&p, &owner, &assets, 2, &cancel, ids(9400))
        .await
        .unwrap();
    assert_eq!(canceled(&out), (4, 3));
    let out = market::cancel(&p, &owner, &assets, 1, &cancel, ids(9500))
        .await
        .unwrap();
    assert_eq!(canceled(&out), (0, 3));
    assert_eq!(count(&p, 1, "synthetic_ore").await, 50);
    let out = market::cancel(&p, &owner, &assets, 1, &cancel, ids(9600))
        .await
        .unwrap();
    assert_eq!(canceled(&out).0, 4);
    let statuses: Vec<i16> =
        sqlx::query_scalar("SELECT status FROM game_trade_item_history WHERE tid=$1 ORDER BY hid")
            .bind(tid)
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(statuses, [1, 2, 3]);
    // Expiry: a closed registration leaves the lists and comes back by mail.
    let ore = ore_id(&p, 1).await;
    let out = market::insert_order(&p, &owner, &assets, 1, &order(ore, 5, 10, 60, 0))
        .await
        .unwrap();
    assert_eq!(order_code(&out), 0);
    sqlx::query("UPDATE game_trade_items SET close_ticks=open_ticks-1 WHERE cid=1")
        .execute(&p)
        .await
        .unwrap();
    let out = market::my_items(&p, &assets, 1).await.unwrap();
    assert!(
        matches!(decode(&out, Kind::TradeMyItemInfos), Message::TradeMyItemInfos(m) if m.result == 7)
    );
    let out = market::get_my_things(&p, &owner, &assets, 1, ids(9700))
        .await
        .unwrap();
    assert_eq!(out.packets.len(), 1, "the SellExpired console count");
    let (kind, _, title, n) = last_mail(&p, 1).await;
    assert_eq!((kind, n), (7, 5));
    assert_eq!(
        title,
        utf16("#GameUI_Heroes_TradeMail_SellExpired_Title,synthetic_ore,5")
    );
    let left: i64 = sqlx::query_scalar("SELECT (SELECT count(*) FROM game_trade_items)+(SELECT count(*) FROM game_trade_items_expired)")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(left, 0);
    // AddTradeItem: at most 20 open registrations of one seller (WebError, nothing kept).
    for n in 0..20 {
        let ore = ore_id(&p, 1).await;
        let out = market::insert_order(&p, &owner, &assets, 1, &order(ore, 1, 10, 60, 0))
            .await
            .unwrap();
        assert_eq!(order_code(&out), 0, "order {n}");
    }
    let before = count(&p, 1, "synthetic_ore").await;
    let ore = ore_id(&p, 1).await;
    let out = market::insert_order(&p, &owner, &assets, 1, &order(ore, 1, 10, 60, 0))
        .await
        .unwrap();
    assert_eq!(order_code(&out), 4);
    assert_eq!(count(&p, 1, "synthetic_ore").await, before);
    let events: Vec<i32> =
        sqlx::query_scalar("SELECT DISTINCT event FROM game_trade_logs ORDER BY event")
            .fetch_all(&p)
            .await
            .unwrap();
    for e in [3, 9, 11, 12, 15, 16, 21, 23, 25, 27] {
        assert!(events.contains(&e), "TradeLog event {e}: {events:?}");
    }
    cleanup_game_pool(a, p, s).await;
}
