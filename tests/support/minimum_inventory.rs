use super::{cleanup_game_pool, isolated_game_pool, minimum_character};
use vindictus_server::{
    infrastructure::{character_lobby::UserKey, minimum_inventory},
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self as wire, Kind, Message},
    },
};
async fn state(p: &sqlx::PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT jsonb_agg(to_jsonb(i) ORDER BY id)FROM game_items i),(SELECT jsonb_agg(to_jsonb(e) ORDER BY id)FROM game_equippables e),(SELECT jsonb_agg(to_jsonb(c) ORDER BY id)FROM game_character_info c),(SELECT jsonb_agg(to_jsonb(u) ORDER BY nexon_sn)FROM game_user_info u),(SELECT jsonb_agg(to_jsonb(m) ORDER BY cid)FROM game_minimum_character_initialization m))::text").fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_inventory_projects_existing_rows_and_repeated_queries_are_read_only() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = minimum_character::assets();
    let key = super::initial_inventory::key();
    let before = state(&p).await;
    let packet = minimum_inventory::query(&p, &key, &assets, 1)
        .await
        .unwrap();
    assert_eq!(packet.category, 468);
    let Some(Message::InventoryInfo(snapshot)) =
        wire::decode(Kind::InventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("inventory missing")
    };
    let stored: Vec<(i64, i32, i16, i32, bool)> =
        sqlx::query_as("SELECT id,count,collection,slot,tradable FROM game_items ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap();
    let rows = snapshot.slots.unwrap();
    assert_eq!(rows.len(), stored.len());
    for (slot, (id, count, tab, position, tradable)) in rows.iter().zip(stored) {
        let slot = slot.as_ref().unwrap();
        assert_eq!(
            (slot.id, slot.count, slot.tab, slot.slot, slot.tradable),
            (id, count, i32::from(tab), position, tradable)
        );
        assert_eq!((slot.expire_ticks, slot.expire_diff), (-1, -1));
        assert!(!slot.expireable);
        assert!(slot.attributes.as_ref().unwrap().is_empty());
        assert_eq!(slot.colors.as_ref().unwrap().len(), 3);
    }
    assert_eq!(
        snapshot
            .storages
            .unwrap()
            .iter()
            .map(|s| s.as_ref().unwrap().id)
            .collect::<Vec<_>>(),
        [0, 1, 6]
    );
    assert!(snapshot.equipment.0.unwrap().is_empty());
    assert!(snapshot.quick_slots.unwrap().classes.0.unwrap().is_empty());
    let repeat = minimum_inventory::query(&p, &key, &assets, 1)
        .await
        .unwrap();
    assert_eq!(repeat.body, packet.body);
    assert_eq!(state(&p).await, before);
    let stale = UserKey {
        revision: key.revision + 1,
        ..key
    };
    assert!(
        minimum_inventory::query(&p, &stale, &assets, 1)
            .await
            .is_err()
    );
    assert!(
        minimum_inventory::query(&p, &key, &assets, 2)
            .await
            .is_err()
    );
    assert_eq!(state(&p).await, before);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_inventory_refuses_partial_unsupported_and_corrupt_state_without_repair() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = minimum_character::assets();
    let key = super::initial_inventory::key();
    for (change, undo) in [
        (
            "UPDATE game_minimum_character_initialization SET initialized=false",
            "UPDATE game_minimum_character_initialization SET initialized=true",
        ),
        (
            "UPDATE game_equippables SET part=1",
            "UPDATE game_equippables SET part=NULL",
        ),
        (
            "INSERT INTO game_expanded_slots(row_id,cid,slot) VALUES(123,1,1)",
            "DELETE FROM game_expanded_slots WHERE row_id=123",
        ),
        (
            "INSERT INTO game_quick_slots(row_id,owner_id,item_class_utf16,slot) VALUES(123,1,decode('4100','hex'),1)",
            "DELETE FROM game_quick_slots WHERE row_id=123",
        ),
        (
            "UPDATE game_items SET checksum=1 WHERE checksum IS NULL",
            "UPDATE game_items SET checksum=NULL",
        ),
        // Equipment attributes feed ItemStat; MIN_UPTIME needs ToFormatableDateTime.
        (
            "INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) SELECT 123,min(i.id),decode('41','hex'),decode('42','hex'),0,0 FROM game_items i JOIN game_equippables e ON e.id=i.id",
            "DELETE FROM game_item_attributes WHERE id=123",
        ),
        (
            "INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) SELECT 123,min(i.id),'MIN_UPTIME','',0,0 FROM game_items i WHERE NOT EXISTS(SELECT 1 FROM game_equippables e WHERE e.id=i.id)",
            "DELETE FROM game_item_attributes WHERE id=123",
        ),
        (
            "INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) SELECT 123,min(i.id),decode('41','hex'),decode('e9','hex'),0,0 FROM game_items i WHERE NOT EXISTS(SELECT 1 FROM game_equippables e WHERE e.id=i.id)",
            "DELETE FROM game_item_attributes WHERE id=123",
        ),
    ] {
        assert!(
            sqlx::query(change)
                .execute(&p)
                .await
                .unwrap()
                .rows_affected()
                > 0,
            "fixture did not modify state: {change}"
        );
        let before = state(&p).await;
        assert!(
            minimum_inventory::query(&p, &key, &assets, 1)
                .await
                .is_err(),
            "unsupported state admitted"
        );
        assert_eq!(state(&p).await, before);
        sqlx::query(undo).execute(&p).await.unwrap();
    }
    assert!(minimum_inventory::query(&p, &key, &assets, 1).await.is_ok());
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_inventory_projects_item_expiry_like_slot_info() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = minimum_character::assets();
    let key = super::initial_inventory::key();
    // Any stored item with ExpireDateTime one hour ahead (SlotInfo projects it for
    // equippable and stackable items alike).
    let id: i64 = sqlx::query_scalar("SELECT min(id) FROM game_items")
        .fetch_one(&p)
        .await
        .unwrap();
    let now = 621_355_968_000_000_000
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64
            / 100;
    let expiry = now + 36_000_000_000;
    sqlx::query("UPDATE game_items SET expire_ticks=$2 WHERE id=$1")
        .bind(id)
        .bind(expiry)
        .execute(&p)
        .await
        .unwrap();
    let packet = minimum_inventory::query(&p, &key, &assets, 1)
        .await
        .unwrap();
    let Some(Message::InventoryInfo(snapshot)) =
        wire::decode(Kind::InventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("inventory missing")
    };
    let slots = snapshot.slots.unwrap();
    let slot = slots.iter().flatten().find(|s| s.id == id).unwrap();
    // SlotInfo: IsExpireable, ExpireDateTime.Ticks and Ticks − UtcNow.Ticks.
    assert!(slot.expireable);
    assert_eq!(slot.expire_ticks, expiry);
    assert!((35_000_000_000..=36_000_000_000).contains(&slot.expire_diff));
    for other in slots.iter().flatten().filter(|s| s.id != id) {
        assert!(!other.expireable);
    }
    cleanup_game_pool(a, p, s).await;
}
