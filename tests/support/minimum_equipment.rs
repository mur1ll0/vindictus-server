use super::{cleanup_game_pool, isolated_game_pool, minimum_character};
use std::sync::Arc;
use vindictus_server::{
    domain::character_stats::{Arithmetic, Field},
    infrastructure::{
        character_lobby::{QueryConfig, UserKey},
        equipment_profile::Profile,
        minimum_assets::Assets,
        minimum_equip, minimum_equipment, minimum_inventory, minimum_selection,
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
fn log(assets: &Assets) -> vindictus_server::infrastructure::minimum_item_logs::Context {
    vindictus_server::infrastructure::minimum_item_logs::Context {
        flags: vindictus_server::infrastructure::minimum_item_logs::flags(assets).unwrap(),
        civil_offset_seconds: 0,
    }
}
fn config() -> QueryConfig {
    QueryConfig {
        max_free: 5,
        care_clock: None,
    }
}
fn fail_format(packet: &vindictus_server::domain::map_partition::OutgoingPacket) -> String {
    assert_eq!(packet.category, 358);
    let Some(Message::ItemFail(Some(s))) =
        inventory::decode(Kind::ItemFail, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("item fail missing")
    };
    assert_eq!(s.parameters, Some(Vec::new()));
    s.format.unwrap()
}
async fn equip(
    p: &sqlx::PgPool,
    owner: &UserKey<'_>,
    assets: &Assets,
    item: i64,
    part: i32,
) -> Result<minimum_equip::Outcome, vindictus_server::infrastructure::minimum_character::MinimumError>
{
    minimum_equip::equip(
        p,
        owner,
        assets,
        1,
        item,
        part,
        config(),
        Arithmetic::FrameworkX64,
        log(assets),
    )
    .await
}
async fn costumes(p: &sqlx::PgPool) -> Vec<(bool, i16, i16, Option<i32>)> {
    sqlx::query_as("SELECT is_base,costume_type,costume_key,costume_sn FROM game_costumes WHERE cid=1 ORDER BY row_id")
        .fetch_all(p)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_equip_saves_syncs_refuses_unequips_and_survives_relogin() {
    use vindictus_server::protocol::{character_list, equipment};
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    let id: i64 = sqlx::query_scalar("SELECT min(id)FROM game_equippables")
        .fetch_one(&p)
        .await
        .unwrap();
    // Not selected: is_connected is false.
    let untouched = state(&p).await;
    assert!(equip(&p, &owner, &assets, id, 1).await.is_err());
    let baseline =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    let base_costumes = costumes(&p).await;
    // Refusals keep every row unchanged and answer with the source dialog.
    let before = state(&p).await;
    for (item, part, expected) in [
        (id, 2, "GameUI_Heroes_EquipItemFail_InvalidSlot"),
        (id, 200, "GameUI_Heroes_EquipItemFail_InvalidSlot"),
        (id, -1, "GameUI_Heroes_EquipItemFail_InvalidSlot"),
        (i64::MAX, 1, "GameUI_Heroes_EquipItemFail_System"),
    ] {
        let Ok(minimum_equip::Outcome::Refused(packet)) =
            equip(&p, &owner, &assets, item, part).await
        else {
            panic!("refusal expected")
        };
        assert_eq!(fail_format(&packet), expected);
        assert_eq!(state(&p).await, before);
    }
    let Ok(minimum_equip::Outcome::Applied { packets, look }) =
        equip(&p, &owner, &assets, id, 1).await
    else {
        panic!("equip expected")
    };
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [617, 458, 619, 447]
    );
    let Some(Message::UpdateInventoryInfo(Some(slots))) = inventory::decode(
        Kind::UpdateInventoryInfo,
        &packets[0].body,
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("slot update missing")
    };
    assert_eq!(
        slots
            .iter()
            .map(|s| s.as_ref().unwrap().id)
            .collect::<Vec<_>>(),
        [id]
    );
    let Some(equipment::Message::EquipmentInfo(info)) = equipment::decode(
        equipment::Kind::EquipmentInfo,
        &packets[1].body,
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("equipment info missing")
    };
    assert_eq!(info.0.unwrap(), vec![(1, id)]);
    let costume = character_list::decode_costume_update(&packets[3].body, DEFAULT_MAX_BODY)
        .unwrap()
        .unwrap()
        .unwrap();
    assert_ne!(packets[2].body, baseline.packets[3].body);
    assert_ne!(look.body, baseline.look.body);
    let part: Option<i16> = sqlx::query_scalar("SELECT part FROM game_equippables WHERE id=$1")
        .bind(id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(part, Some(1));
    let equipped_costumes = costumes(&p).await;
    assert_ne!(equipped_costumes, base_costumes);
    assert!(
        equipped_costumes
            .iter()
            .any(|c| !c.0 && c.2 == 4 && c.3 == Some(29))
    );
    // Equipping the same item to its own part is CheckSettable's IsEquipped failure.
    let before = state(&p).await;
    let Ok(minimum_equip::Outcome::Refused(packet)) = equip(&p, &owner, &assets, id, 1).await
    else {
        panic!("refusal expected")
    };
    assert_eq!(fail_format(&packet), "GameUI_Heroes_EquipItemFail_System");
    assert_eq!(state(&p).await, before);
    // Relogin reproduces the synchronized stats, look and CostumeInfo.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    let again =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    assert_eq!(again.packets[3].body, packets[2].body);
    assert_eq!(again.look.body, look.body);
    let Some(character_list_info) = vindictus_server::protocol::minimum_map::decode(
        vindictus_server::protocol::minimum_map::Kind::CharacterCommonInfo,
        &again.packets[2].body,
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("common info missing")
    };
    let vindictus_server::protocol::minimum_map::Message::CharacterCommonInfo {
        info: Some(summary),
        ..
    } = character_list_info
    else {
        panic!("summary missing")
    };
    assert!(summary.costume.as_ref() == Some(&costume));
    // Unequip clears the part, deletes the item costume row and restores the base view.
    let Ok(minimum_equip::Outcome::Applied {
        packets: off,
        look: off_look,
    }) = equip(&p, &owner, &assets, -1, 1).await
    else {
        panic!("unequip expected")
    };
    assert_eq!(
        off.iter().map(|p| p.category).collect::<Vec<_>>(),
        [617, 458, 619, 447]
    );
    assert_eq!(off[2].body, baseline.packets[3].body);
    assert_eq!(off_look.body, baseline.look.body);
    assert_eq!(costumes(&p).await, base_costumes);
    let part: Option<i16> = sqlx::query_scalar("SELECT part FROM game_equippables WHERE id=$1")
        .bind(id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(part, None);
    // Empty slot and unknown part: nothing saved; the unknown part raises in the source.
    let before = state(&p).await;
    assert!(matches!(
        equip(&p, &owner, &assets, -1, 1).await,
        Ok(minimum_equip::Outcome::Unchanged)
    ));
    assert!(equip(&p, &owner, &assets, -1, 200).await.is_err());
    let stale = UserKey {
        uid: owner.uid,
        name: owner.name,
        revision: owner.revision + 1,
    };
    assert!(
        minimum_equip::equip(
            &p,
            &stale,
            &assets,
            1,
            id,
            1,
            config(),
            Arithmetic::FrameworkX64,
            log(&assets)
        )
        .await
        .is_err()
    );
    assert_eq!(state(&p).await, before);
    minimum_selection::disconnect(&p, 1).await.unwrap();
    // The full equip, relogin and unequip cycle restores every persisted row.
    assert_eq!(state(&p).await, untouched);
    cleanup_game_pool(a, p, s).await;
}
async fn positions(p: &sqlx::PgPool) -> Vec<(i64, i16, i32)> {
    sqlx::query_as("SELECT id,collection,slot FROM game_items WHERE owner_id=1 ORDER BY id")
        .fetch_all(p)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_move_swaps_bag_slots_refuses_source_failures_and_survives_relogin() {
    use vindictus_server::{infrastructure::minimum_move, protocol::inventory::Move};
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    let id: i64 = sqlx::query_scalar("SELECT min(id)FROM game_equippables")
        .fetch_one(&p)
        .await
        .unwrap();
    // A second bag item of the same equippable class (not stackable) for the swap.
    let other = id + 1;
    sqlx::query("INSERT INTO game_items(id,item_class_utf16,owner_id,count,collection,slot,expire_ticks,tradable,checksum) SELECT $2,item_class_utf16,owner_id,count,collection,7,expire_ticks,tradable,checksum FROM game_items WHERE id=$1").bind(id).bind(other).execute(&p).await.unwrap();
    sqlx::query("INSERT INTO game_equippables SELECT $2,part,color1,color2,color3,reduced_durability,max_durability_bonus FROM game_equippables WHERE id=$1").bind(id).bind(other).execute(&p).await.unwrap();
    let request = |item_id, storage, slot| Move {
        item_id,
        storage,
        slot,
    };
    let mv = |r| minimum_move::move_item(&p, &owner, &assets, 1, r, log(&assets));
    assert!(mv(request(id, 0, 5)).await.is_err());
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let decode = |packet: &vindictus_server::domain::map_partition::OutgoingPacket| {
        assert_eq!(packet.category, 617);
        let Some(Message::UpdateInventoryInfo(Some(slots))) =
            inventory::decode(Kind::UpdateInventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("slot update missing")
        };
        slots
            .into_iter()
            .map(|s| {
                let s = s.unwrap();
                (s.id, s.tab, s.slot)
            })
            .collect::<Vec<_>>()
    };
    let Ok(minimum_move::Outcome::Applied(packet)) = mv(request(id, 0, 5)).await else {
        panic!("move expected")
    };
    assert_eq!(decode(&packet), [(id, 0, 5)]);
    // Swap: the moved item takes the target, the occupant takes the source slot.
    let Ok(minimum_move::Outcome::Applied(packet)) = mv(request(id, 0, 7)).await else {
        panic!("swap expected")
    };
    assert_eq!(decode(&packet), [(id, 0, 7), (other, 0, 5)]);
    let before = state(&p).await;
    for r in [
        request(id, 0, 48),
        request(id, 0, -1),
        request(id, 2, 0),
        request(id, 100, 1),
        request(i64::MAX, 0, 1),
    ] {
        let Ok(minimum_move::Outcome::Refused(packet)) = mv(r).await else {
            panic!("refusal expected")
        };
        assert_eq!(packet.category, 602);
        assert_eq!(state(&p).await, before);
    }
    assert!(matches!(
        mv(request(id, 0, 7)).await,
        Ok(minimum_move::Outcome::Unchanged)
    ));
    // Cash storage moves are outside this slice and change nothing.
    let cash: i64 = sqlx::query_scalar("SELECT id FROM game_items WHERE collection=100")
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(mv(request(cash, 100, 3)).await.is_err());
    assert_eq!(state(&p).await, before);
    let moved = positions(&p).await;
    minimum_selection::disconnect(&p, 1).await.unwrap();
    let again =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    let Some(Message::InventoryInfo(info)) = inventory::decode(
        Kind::InventoryInfo,
        &again.packets.last().unwrap().body,
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("inventory missing")
    };
    let mut relogin: Vec<(i64, i16, i32)> = info
        .slots
        .unwrap()
        .into_iter()
        .map(|s| {
            let s = s.unwrap();
            (s.id, s.tab as i16, s.slot)
        })
        .collect();
    relogin.sort();
    assert_eq!(relogin, moved);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_ap_query_reports_stored_ap_and_unsupported_use_changes_nothing() {
    use vindictus_server::{
        infrastructure::{minimum_ap, minimum_use},
        protocol::inventory::UseItem,
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    // Not selected: is_connected is false.
    assert!(
        minimum_ap::query(&p, &owner, &assets, 1, Arithmetic::FrameworkX64)
            .await
            .is_err()
    );
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    sqlx::query("UPDATE game_character_info SET ap=37 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    let before = state(&p).await;
    let packet = minimum_ap::query(&p, &owner, &assets, 1, Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(packet.category, 229);
    let Some(Message::Ap(ap)) =
        inventory::decode(Kind::Ap, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("AP message missing")
    };
    // Untrained meditation: BonusAPTime 0 reports -1 for both timing fields.
    assert_eq!((ap.ap, ap.next_bonus_ms, ap.bonus_interval), (37, -1, -1));
    assert_eq!(state(&p).await, before);
    // Non-capsule items and unknown IDs change nothing.
    let cash: i64 = sqlx::query_scalar("SELECT id FROM game_items WHERE collection=100")
        .fetch_one(&p)
        .await
        .unwrap();
    let use_item = |item_id| {
        minimum_use::use_item(
            &p,
            &owner,
            &assets,
            1,
            UseItem {
                item_id,
                target_item_id: -1,
            },
            log(&assets),
            vindictus_server::infrastructure::minimum_quest::QuestClock {
                arithmetic: vindictus_server::domain::character_stats::Arithmetic::FrameworkX64,
                civil_offset_seconds: 0,
            },
            &[],
            Vec::new(),
        )
    };
    assert!(use_item(cash).await.is_err());
    assert!(matches!(
        use_item(i64::MAX).await,
        Ok(minimum_use::Outcome::Unchanged)
    ));
    assert_eq!(state(&p).await, before);
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_skill_list_projects_stored_skills_without_writes() {
    use vindictus_server::infrastructure::{minimum_skills, skill_profile};
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let stored: Vec<(Vec<u8>, i32)> =
        sqlx::query_as("SELECT skill_utf16,rank FROM game_skills WHERE cid=1 ORDER BY row_id")
            .fetch_all(&p)
            .await
            .unwrap();
    assert!(!stored.is_empty());
    let sources: serde_json::Value =
        serde_json::from_str(include_str!("../../src/domain/skill_list_sources.json")).unwrap();
    let implementations: serde_json::Map<String, serde_json::Value> = sources
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v["sha256"].clone()))
        .collect();
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mut skills = Vec::new();
    let mut ranks = Vec::new();
    for (n, (key, rank)) in stored.iter().enumerate() {
        skills.push(
            serde_json::json!({"source_index":n,"skill_utf16":hex(key),"class_restriction":2,"use_type":1,"price":0}),
        );
        for (r, ap) in [(*rank, 0), (rank + 1, 30)] {
            ranks.push(serde_json::json!({"source_index":ranks.len(),"skill_utf16":hex(key),"rank":r,"level":1,"required_ap":ap,"require_unlock":false,"feature":null}));
        }
    }
    let profile = serde_json::json!({"format_version":4,"source":"heroesContents.skill-ranks","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"implementations":implementations,"skills":skills,"ranks":ranks,"constraints":[],"arguments":[]});
    let assets = assets()
        .with_skills(
            skill_profile::Profile::read(serde_json::to_vec(&profile).unwrap().as_slice()).unwrap(),
        )
        .unwrap();
    let owner = super::initial_inventory::key();
    assert!(minimum_skills::query(&p, &owner, &assets, 1).await.is_err());
    let selected =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    // InGame.OnEnter: Stat observer, Skill observer, QueryAP, then the inventory sync.
    assert_eq!(
        selected
            .packets
            .iter()
            .map(|p| p.category)
            .collect::<Vec<_>>(),
        [483, 345, 609, 619, 583, 229, 468]
    );
    let before = state(&p).await;
    let packet = minimum_skills::query(&p, &owner, &assets, 1).await.unwrap();
    assert_eq!(packet.category, 583);
    let Some(Message::SkillList(list)) =
        inventory::decode(Kind::SkillList, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("skill list missing")
    };
    assert_eq!(
        (
            list.learning.as_deref(),
            list.current_ap,
            list.reset_vocation
        ),
        (Some(""), 0, false)
    );
    let skills = list.skills.unwrap();
    assert_eq!(skills.len(), stored.len());
    for (b, (_, rank)) in skills.iter().zip(&stored) {
        let b = b.as_ref().unwrap();
        assert_eq!(
            (
                b.base_rank,
                b.final_rank,
                b.required_ap,
                b.can_start_training
            ),
            (*rank, *rank, 30, 1)
        );
        assert!(b.enhances == Some(Vec::new()));
    }
    assert_eq!(state(&p).await, before);
    // QueryCharacterCommonInfo for the own CID answers the summary with the QueryID.
    let common = minimum_selection::common_info(
        &p,
        &owner,
        &assets,
        1,
        77,
        config(),
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    assert_eq!(common.category, 609);
    // LearnSkill: partial training, then completion with a refund of the excess AP.
    let first = String::from_utf16(
        &stored[0]
            .0
            .as_chunks::<2>()
            .0
            .iter()
            .map(|a| u16::from_le_bytes(*a))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    sqlx::query("UPDATE game_character_info SET ap=40,total_used_ap=0 WHERE id=1")
        .execute(&p)
        .await
        .unwrap();
    assert!(
        minimum_skills::learn(&p, &owner, &assets, 1, &first, 41, Arithmetic::FrameworkX64)
            .await
            .unwrap()
            .is_empty()
    );
    let partial =
        minimum_skills::learn(&p, &owner, &assets, 1, &first, 20, Arithmetic::FrameworkX64)
            .await
            .unwrap();
    assert_eq!(
        partial.iter().map(|p| p.category).collect::<Vec<_>>(),
        [229, 583]
    );
    let done = minimum_skills::learn(&p, &owner, &assets, 1, &first, 20, Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(
        done.iter().map(|p| p.category).collect::<Vec<_>>(),
        [583, 229, 583, 582]
    );
    let (ap, used, rank, current): (i32, i32, i32, i32) = sqlx::query_as("SELECT c.ap,c.total_used_ap,s.rank,s.current_ap FROM game_character_info c JOIN game_skills s ON s.cid=c.id WHERE c.id=1 AND s.skill_utf16=$1").bind(&stored[0].0).fetch_one(&p).await.unwrap();
    assert_eq!((ap, used, rank, current), (10, 30, stored[0].1 + 1, 0));
    // Relogin loads the trained rank from the content profile.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    cleanup_game_pool(a, p, s).await;
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_hide_head_costume_persists_and_pushes_costume_update() {
    use vindictus_server::{
        infrastructure::minimum_costume,
        protocol::{character_list, inventory::HideCostume},
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = assets();
    let owner = super::initial_inventory::key();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let hide = |hide_head, avatar_part| {
        minimum_costume::hide_costume(
            &p,
            &owner,
            &assets,
            1,
            HideCostume {
                hide_head,
                avatar_part,
                avatar_state: -1,
            },
            config(),
            Arithmetic::FrameworkX64,
        )
    };
    let rows = || async {
        sqlx::query_as::<_, (bool, i16, i16, Option<i32>)>(
            "SELECT is_base,costume_type,costume_key,costume_sn FROM game_costumes WHERE cid=1 AND costume_type=2 AND costume_key=7",
        )
        .fetch_all(&p)
        .await
        .unwrap()
    };
    let flag = |applied: &minimum_costume::Applied| {
        let packet = &applied.packets[0];
        assert_eq!(packet.category, character_list::COSTUME_UPDATE_CATEGORY);
        character_list::decode_costume_update(&packet.body, DEFAULT_MAX_BODY)
            .unwrap()
            .flatten()
            .unwrap()
            .hide_head_costume
    };
    // Showing an already shown helmet changes nothing and sends nothing.
    assert!(hide(false, -1).await.unwrap().is_none());
    assert!(rows().await.is_empty());
    // LoadExtraData(HideHeadCostume, 1): a base Type 2 / Key 7 row, CostumeUpdate.
    let hidden = hide(true, -1).await.unwrap().unwrap();
    assert!(flag(&hidden));
    assert_eq!(rows().await, [(true, 2, 7, Some(1))]);
    assert!(hide(true, -1).await.unwrap().is_none());
    // This catalog has no AvatarHide feature: the source skips the avatar part branch,
    // so only HideHead counts (already hidden: nothing changes).
    assert!(hide(true, 54).await.unwrap().is_none());
    assert_eq!(rows().await, [(true, 2, 7, Some(1))]);
    let shown = hide(false, -1).await.unwrap().unwrap();
    assert!(!flag(&shown));
    assert_eq!(rows().await, [(true, 2, 7, Some(0))]);
    hide(true, -1).await.unwrap().unwrap();
    // Relogin keeps the stored flag, and the next toggle starts from it.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(rows().await, [(true, 2, 7, Some(1))]);
    assert!(hide(true, -1).await.unwrap().is_none());
    cleanup_game_pool(a, p, s).await;
}

fn plain_assets() -> Assets {
    assets()
}

pub(super) fn enhance_profile() -> vindictus_server::infrastructure::item_enhance::Profile {
    enhance_profile_for(&super::fiona_inventory_delivery::appearance_fixture::fixtures().0)
}

/// The synthetic enhance profile aligned with `catalog`'s equipment and class rows.
pub(super) fn enhance_profile_for(
    catalog: &serde_json::Value,
) -> vindictus_server::infrastructure::item_enhance::Profile {
    use serde_json::json;
    let key = |s: &str| -> String {
        s.encode_utf16()
            .flat_map(u16::to_le_bytes)
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    let single = |v: f32| format!("{:08x}", v.to_bits());
    let equips: Vec<serde_json::Value> = catalog["equips"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, e)| json!({"source_index":i,"item_utf16":e["item_utf16"],"feature":e["feature"],"enhance_type":key("BAG"),"enchantable":1,"quality_type":null}))
        .collect();
    let info = |i: usize, level: i32, effect: i32| json!({"source_index":i,"enhance_type":key("BAG"),"level":level,"material1":key("stone"),"num1":1,"material2":null,"num2":0,"gold":100,"success_ratio":single(0.5),"on_fail":key("NoPenalty"),"on_fail_recipe_ratio":single(0.0),"on_fail_material_num1":0,"effect":effect,"feature":null});
    let stat = |i: usize, level: i32, stat: &str, kind: &str, value: f32| json!({"source_index":i,"enhance_type":key("BAG"),"level":level,"stat":key(stat),"value_type":key(kind),"value":single(value),"base":key("MaxDurability")});
    let classes: Vec<serde_json::Value> = catalog["classes"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .map(|(i, c)| json!({"source_index":i,"item_utf16":c["item_utf16"],"feature":c["feature"],"max_anti_bind":2}))
        .collect();
    let profile = json!({"format_version":7,"source":"heroesContents.item-enhance","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"equips":equips,
        "enhance":[info(0,3,7),info(1,4,8),info(2,5,9)],
        // Every base stat of the fixture equipment is 0 except MaxDurability (200).
        "stats":[stat(0,3,"MaxDurability","Add",300.0),stat(1,3,"ATK","Add",50.0),stat(2,4,"DEF_Destroyed","Multiply",0.35)],
        "server":[],"bonus":[],"instead":[],"classes":classes,"runes":[],"rune_extra":[],"recipes":[],"anti_bind":[],"enchant":[],"enchant_stats":[],"enchant_dice":[],"manufacture":[],"manufacture_recipes":[],"manufacture_materials":[],"manufacture_dependencies":[],"quality_dice":[],"quality_stats":[],"skill_enhance":[],"skill_enhance_materials":[],"skill_enhance_groups":[],"features":[]});
    vindictus_server::infrastructure::item_enhance::Profile::read(
        serde_json::to_vec(&profile).unwrap().as_slice(),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_enhanced_equipment_projects_item_stat_effect_and_durability() {
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let plain = assets();
    let assets = assets().with_enhance(enhance_profile()).unwrap();
    let owner = super::initial_inventory::key();
    let id = equipped(&p).await;
    let base = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let base_info = base
        .projection
        .infos
        .iter()
        .find(|i| i.part == 1)
        .unwrap()
        .clone();
    assert_eq!(base_info.effect, None);
    let set = |value: &'static str| {
        let p = &p;
        async move {
            sqlx::query("DELETE FROM game_item_attributes WHERE item_id=$1")
                .bind(id)
                .execute(p)
                .await
                .unwrap();
            sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(nextval('game_item_attribute_id'),$1,'ENHANCE',$2,0,0)")
                .bind(id)
                .bind(value.as_bytes())
                .execute(p)
                .await
                .unwrap();
        }
    };
    // ENHANCE 3: Item.Stat = class stat + Enhance.Stat; Effect from EnhanceInfo.
    set("3").await;
    let gear = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let info = gear.projection.infos.iter().find(|i| i.part == 1).unwrap();
    assert_eq!(info.effect, Some(7));
    assert_eq!(info.maximum, base_info.maximum + 300);
    assert_eq!(
        info.stats.get(Field::Atk),
        base_info.stats.get(Field::Atk) + 50
    );
    let packet = minimum_inventory::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let Some(Message::InventoryInfo(inventory)) =
        inventory::decode(Kind::InventoryInfo, &packet.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("inventory missing")
    };
    let slot = inventory
        .slots
        .unwrap()
        .into_iter()
        .flatten()
        .find(|s| s.id == id)
        .unwrap();
    assert_eq!(slot.maximum, info.maximum);
    assert_eq!(
        slot.attributes.unwrap()[0]
            .as_ref()
            .unwrap()
            .value
            .as_deref(),
        Some("3")
    );
    // ENHANCE 4: (int)((float)200 × 0.35f) is 70 on x64 and 69 on x86. Without the
    // runtime's arithmetic the disagreement refuses the item.
    set("4").await;
    assert!(
        minimum_equipment::query(&p, &owner, &assets, 1)
            .await
            .is_err()
    );
    assert!(
        minimum_inventory::query(&p, &owner, &assets, 1)
            .await
            .is_err()
    );
    let x64 = plain_assets()
        .with_enhance(enhance_profile())
        .unwrap()
        .with_item_arithmetic(Arithmetic::FrameworkX64);
    let x86 = plain_assets()
        .with_enhance(enhance_profile())
        .unwrap()
        .with_item_arithmetic(Arithmetic::FrameworkX86);
    for (assets, destroyed) in [(&x64, 70), (&x86, 69)] {
        let gear = minimum_equipment::query(&p, &owner, assets, 1)
            .await
            .unwrap();
        let info = gear.projection.infos.iter().find(|i| i.part == 1).unwrap();
        assert_eq!((info.destroyed_def, info.effect), (destroyed, Some(8)));
    }
    // ENHANCE 5 has EnhanceInfo without EnhanceStatInfo (NullReference in the source);
    // "0" leaves Level 0 without an info; other equipment attributes are refused.
    set("5").await;
    assert!(minimum_equipment::query(&p, &owner, &x64, 1).await.is_err());
    set("0").await;
    let zero = minimum_equipment::query(&p, &owner, &assets, 1)
        .await
        .unwrap();
    let info = zero.projection.infos.iter().find(|i| i.part == 1).unwrap();
    assert_eq!((info.effect, info.maximum), (None, base_info.maximum));
    sqlx::query("INSERT INTO game_item_attributes(id,item_id,attribute_bytes,value_bytes,arg,arg2) VALUES(nextval('game_item_attribute_id'),$1,'ENHANCEOTHER','',5,0)")
        .bind(id)
        .execute(&p)
        .await
        .unwrap();
    assert!(minimum_equipment::query(&p, &owner, &x64, 1).await.is_err());
    // Without the enhance profile an ENHANCE item cannot be projected.
    set("3").await;
    assert!(
        minimum_equipment::query(&p, &owner, &plain, 1)
            .await
            .is_err()
    );
    cleanup_game_pool(a, p, s).await;
}
