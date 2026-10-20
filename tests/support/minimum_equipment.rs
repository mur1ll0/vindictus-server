use super::{cleanup_game_pool, isolated_game_pool, minimum_character};
use std::sync::Arc;
use vindictus_server::{
    domain::character_stats::{Arithmetic, Field},
    infrastructure::{
        character_lobby::{QueryConfig, UserKey},
        equipment_profile::Profile,
        minimum_assets::Assets,
        minimum_equipment, minimum_inventory, minimum_selection,
    },
    protocol::{
        DEFAULT_MAX_BODY,
        inventory::{self, Kind, Message},
    },
};
include!("../fixtures/equipment_profile_support.rs");
fn assets() -> Assets {
    Arc::try_unwrap(minimum_character::assets())
        .ok()
        .unwrap()
        .with_equipment(
            Profile::read(
                serde_json::to_vec(&equipment_profile_fixture())
                    .unwrap()
                    .as_slice(),
            )
            .unwrap(),
        )
        .unwrap()
}
async fn equipped(p: &sqlx::PgPool) -> i64 {
    let id: i64 = sqlx::query_scalar("SELECT min(id)FROM game_equippables")
        .fetch_one(p)
        .await
        .unwrap();
    sqlx::query("UPDATE game_equippables SET part=1,color1=31,color2=32,color3=33 WHERE id=$1")
        .bind(id)
        .execute(p)
        .await
        .unwrap();
    id
}
async fn state(p: &sqlx::PgPool) -> String {
    sqlx::query_scalar("SELECT jsonb_build_array((SELECT jsonb_agg(to_jsonb(e)ORDER BY id)FROM game_equippables e),(SELECT jsonb_agg(to_jsonb(i)ORDER BY id)FROM game_items i),(SELECT jsonb_agg(to_jsonb(c)ORDER BY row_id)FROM game_costumes c),(SELECT jsonb_agg(to_jsonb(c)ORDER BY id)FROM game_character_info c))::text").fetch_one(p).await.unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_equipment_reads_persisted_parts_set_bonus_and_inventory_without_writes() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    let id = equipped(&p).await;
    let before = state(&p).await;
    let gear = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    assert_eq!(gear.projection.equipment, vec![(1, id)]);
    assert_eq!(gear.bonus.stats.get(Field::Def), 7);
    assert!(gear.bonus.skills.is_empty());
    assert_eq!(gear.projection.infos[0].colors, [31, 32, 33]);
    assert_eq!(gear.projection.infos[0].serial, 29);
    let packet = minimum_inventory::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let Some(Message::InventoryInfo(info)) =
        inventory::decode(Kind::InventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("inventory missing")
    };
    assert_eq!(info.equipment.0.unwrap(), vec![(1, id)]);
    assert_eq!(state(&p).await, before);
    let repeat = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    assert_eq!(repeat.projection.equipment, gear.projection.equipment);
    assert_eq!(state(&p).await, before);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_equipment_selection_reconstructs_look_stats_and_relogin_without_item_repair() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    let baseline =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    minimum_selection::disconnect(&p, 1).await.unwrap();
    let id = equipped(&p).await;
    let before: Vec<(i64, Option<i16>)> =
        sqlx::query_as("SELECT id,part FROM game_equippables ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap();
    let selected =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    assert_ne!(selected.look.body, baseline.look.body);
    assert_ne!(selected.packets[3].body, baseline.packets[3].body);
    assert_eq!(selected.packets.last().unwrap().category, 468);
    minimum_selection::disconnect(&p, 1).await.unwrap();
    let again =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    assert_eq!(again.look.body, selected.look.body);
    assert_eq!(again.packets[3].body, selected.packets[3].body);
    minimum_selection::disconnect(&p, 1).await.unwrap();
    assert_eq!(
        sqlx::query_as::<_, (i64, Option<i16>)>("SELECT id,part FROM game_equippables ORDER BY id")
            .fetch_all(&p)
            .await
            .unwrap(),
        before
    );
    assert!(before.contains(&(id, Some(1))));
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_equipment_refuses_unsupported_states_and_preserves_disabled_equipment_mapping() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    let id = equipped(&p).await;
    for part in [49_i16, 50, 99, 255] {
        sqlx::query("UPDATE game_equippables SET part=$1 WHERE id=$2")
            .bind(part)
            .bind(id)
            .execute(&p)
            .await
            .unwrap();
        let before = state(&p).await;
        assert!(
            minimum_equipment::query(&p, &owner, &assets, 1)
                .await
                .is_err()
        );
        assert_eq!(state(&p).await, before);
    }
    sqlx::query("UPDATE game_equippables SET part=1,reduced_durability=100000 WHERE id=$1")
        .bind(id)
        .execute(&p)
        .await
        .unwrap();
    let gear = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    assert_eq!(gear.projection.equipment, vec![(1, id)]);
    assert!(gear.projection.infos.iter().all(|i| i.class.is_empty()));
    assert_eq!(gear.bonus.stats.get(Field::Def), 0);
    let stale = UserKey {
        uid: owner.uid,
        name: owner.name,
        revision: owner.revision + 1,
    };
    assert!(
        minimum_equipment::query(&p, &stale, &assets, 1)
            .await
            .is_err()
    );
    assert!(
        minimum_equipment::query(&p, &owner, &assets, 999)
            .await
            .is_err()
    );
    cleanup_game_pool(a, p, s).await;
}
