use super::{
    cleanup_game_pool, fiona_inventory_delivery::appearance_fixture, isolated_game_pool,
    minimum_character,
};
use serde_json::{Value, json};
use vindictus_server::{
    domain::give_item::GiveError,
    infrastructure::{
        item_catalog, item_economy,
        minimum_assets::{Assets, Parts},
        minimum_give, minimum_inventory,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self, Kind, Message},
    },
};
/// Fixture assets plus a source-shaped Account gold class and the economy rows.
fn assets(cash_unique: i32) -> Assets {
    let i = super::fiona_inventory_delivery::AppearanceInputs::new();
    let (mut catalog, _, _, mut profile) = appearance_fixture::fixtures();
    let gold = appearance_fixture::key("synthetic_gold");
    profile["classes"].as_array_mut().unwrap().push(json!({"source_index":2,"item_utf16":gold,"feature":null,"category_utf16":null,"unique":0,"expire_in":null}));
    catalog["classes"].as_array_mut().unwrap().push(json!({"item_utf16":gold,"feature":null,"max_stack":2100000000,"storage":3,"bind":null,"trade_on_bind":null}));
    catalog["manifest"]["class_rows"] = json!(3);
    let rows: Vec<Value> = catalog["classes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(n, c)| json!({"item_utf16":c["item_utf16"],"category_utf16":null,"unique":if n == 1 { cash_unique } else { 0 },"expire_in":null,"price":0,"sell_price":0,"required_level":0,"class_restriction":0,"use_in_town":0}))
        .collect();
    let economy = json!({"format_version":1,"source":"heroesContents.item-economy","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"class_rows":3},"classes":rows});
    Assets::from_parts(Parts {
        initializer: super::fiona_inventory_delivery::profile(false),
        catalog: item_catalog::read_package(serde_json::to_vec(&catalog).unwrap().as_slice())
            .unwrap(),
        stats: i.stats,
        colors: i.colors,
        inventory: vindictus_server::infrastructure::fiona_inventory_profile::Profile::read(
            serde_json::to_vec(&profile).unwrap().as_slice(),
        )
        .unwrap(),
        appearance: i.profile,
        entity_service_id: 77,
        arithmetic: None,
        color_seed: 17,
    })
    .unwrap()
    .with_economy(
        item_economy::Profile::read(serde_json::to_vec(&economy).unwrap().as_slice()).unwrap(),
    )
    .unwrap()
}
async fn items(p: &sqlx::PgPool) -> Vec<(i64, Vec<u8>, i32, i16, i32, Option<i32>)> {
    sqlx::query_as("SELECT id,item_class_utf16,count,collection,slot,checksum FROM game_items WHERE owner_id=1 ORDER BY id")
        .fetch_all(p)
        .await
        .unwrap()
}
fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_give_grants_gold_stacks_and_refuses_without_partial_writes() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets(0);
    let owner = super::initial_inventory::key();
    let grant =
        |class: &'static str, count| minimum_give::lab_grant(&p, &assets, None, class, count, 9);
    // Gold: one AccountStorage item with the count checksum, then stacked on it.
    assert_eq!(grant("synthetic_gold", 1500).await.unwrap(), Ok(1));
    assert_eq!(grant("synthetic_gold", 250).await.unwrap(), Ok(1));
    let gold: Vec<_> = items(&p)
        .await
        .into_iter()
        .filter(|r| r.1 == utf16("synthetic_gold"))
        .collect();
    assert_eq!(gold.len(), 1);
    let (id, _, count, collection, slot, checksum) = gold[0].clone();
    assert_eq!((count, collection, slot), (1750, 101, -1));
    let units: Vec<u16> = "synthetic_gold".encode_utf16().collect();
    assert_eq!(
        checksum,
        Some(vindictus_server::domain::initial_inventory::gold_checksum(
            id,
            Some(1),
            &units,
            1750
        ))
    );
    // The selected-character InventoryInfo carries the gold slot at tab 101 / slot -1.
    let packet = minimum_inventory::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let Some(Message::InventoryInfo(info)) =
        inventory::decode(Kind::InventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("inventory missing")
    };
    let inventory: Vec<_> = info
        .slots
        .unwrap()
        .into_iter()
        .flatten()
        .map(|s| (s.id, s.tab, s.slot, s.count))
        .collect();
    assert!(
        inventory
            .iter()
            .any(|s| s.0 == id && s.1 == 101 && s.2 == -1 && s.3 == 1750)
    );
    // Cash stack (MaxStack 9): fills the existing stack, then opens a new cash slot.
    let cash: Vec<_> = items(&p).await.into_iter().filter(|r| r.3 == 100).collect();
    let held: i32 = cash.iter().map(|r| r.2).sum();
    assert_eq!(grant("synthetic_cash", 9).await.unwrap(), Ok(2));
    let after: Vec<_> = items(&p).await.into_iter().filter(|r| r.3 == 100).collect();
    assert_eq!(after.iter().map(|r| r.2).sum::<i32>(), held + 9);
    assert_eq!(after.len(), cash.len() + 1);
    // Equipment: new bag item with an equippable row and no part.
    assert_eq!(grant("synthetic_bag", 1).await.unwrap(), Ok(1));
    let before: Vec<_> = items(&p).await;
    // No empty bag slot for 200 non-stackable items: nothing changes.
    assert_eq!(
        grant("synthetic_bag", 200).await.unwrap(),
        Err(GiveError::NoEmptySlot)
    );
    assert_eq!(
        grant("unknown_class", 1).await.unwrap(),
        Err(GiveError::InvalidItemClass)
    );
    assert_eq!(items(&p).await, before);
    let equippable: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM game_equippables e JOIN game_items i ON i.id=e.id WHERE i.owner_id=1 AND e.part IS NULL",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert!(equippable >= 1);
    // Online characters are refused (the client would not see the change).
    sqlx::query("UPDATE game_character_info SET is_connected=true WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(grant("synthetic_gold", 1).await.is_err());
    assert_eq!(items(&p).await, before);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_give_enforces_the_unique_constraint() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let held: i64 =
        sqlx::query_scalar("SELECT sum(count) FROM game_items WHERE owner_id=1 AND collection=100")
            .fetch_one(&p)
            .await
            .unwrap();
    let assets = assets(held as i32 + 1);
    let before = items(&p).await;
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_cash", 2, 9)
            .await
            .unwrap(),
        Err(GiveError::UniqueConstraint)
    );
    assert_eq!(items(&p).await, before);
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_cash", 1, 9)
            .await
            .unwrap(),
        Ok(1)
    );
    cleanup_game_pool(a, p, s).await;
}
