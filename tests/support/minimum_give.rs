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
    assets_with(cash_unique, 0, 0)
}
/// `assets` with UseInTown and RequiredLevel of the cash class.
fn assets_with(cash_unique: i32, cash_use_in_town: i32, cash_level: i32) -> Assets {
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
        .map(|(n, c)| json!({"item_utf16":c["item_utf16"],"category_utf16":null,"unique":if n == 1 { cash_unique } else { 0 },"expire_in":null,"price":0,"sell_price":if n == 1 { 7 } else { 0 },"required_level":if n == 1 { cash_level } else { 0 },"class_restriction":if n == 1 { 2047 } else { 0 },"use_in_town":if n == 1 { cash_use_in_town } else { 0 },"indestructible":0,"level_upper_bound":0,"cafe_only":0,"required_skill_utf16":null,"required_skill_rank":0,"required_enhance":0,"required_enchant":0}))
        .collect();
    let economy = json!({"format_version":3,"source":"heroesContents.item-economy","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"class_rows":3},"classes":rows});
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
mod equipment_fixture {
    include!("../fixtures/equipment_profile_support.rs");
}
/// Gold/economy assets plus the equipment profile and a skill profile holding the
/// stored skills and one buyable active skill.
async fn skill_assets(p: &sqlx::PgPool, price: i32) -> Assets {
    use vindictus_server::infrastructure::{equipment_profile, skill_profile};
    let stored: Vec<(Vec<u8>, i32)> =
        sqlx::query_as("SELECT skill_utf16,rank FROM game_skills WHERE cid=1 ORDER BY row_id")
            .fetch_all(p)
            .await
            .unwrap();
    let sources: Value =
        serde_json::from_str(include_str!("../../src/domain/skill_list_sources.json")).unwrap();
    let implementations: serde_json::Map<String, Value> = sources
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, v)| (k.clone(), v["sha256"].clone()))
        .collect();
    let hex = |b: &[u8]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    let mut skills = Vec::new();
    let mut ranks = Vec::new();
    let mut keys: Vec<(Vec<u8>, i32, i32)> =
        stored.iter().map(|(k, r)| (k.clone(), *r, 0)).collect();
    keys.push((utf16("skill_new_synthetic"), 1, price));
    for (n, (key, rank, cost)) in keys.iter().enumerate() {
        skills.push(json!({"source_index":n,"skill_utf16":hex(key),"class_restriction":2,"use_type":1,"price":cost}));
        for (r, ap) in [(*rank, 0), (rank + 1, 30)] {
            ranks.push(json!({"source_index":ranks.len(),"skill_utf16":hex(key),"rank":r,"level":1,"required_ap":ap,"require_unlock":false,"feature":null}));
        }
    }
    let profile = json!({"format_version":4,"source":"heroesContents.skill-ranks","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"implementations":implementations,"skills":skills,"ranks":ranks,"constraints":[],"arguments":[]});
    assets(0)
        .with_equipment(
            equipment_profile::Profile::read(
                serde_json::to_vec(&equipment_fixture::equipment_profile_fixture())
                    .unwrap()
                    .as_slice(),
            )
            .unwrap(),
        )
        .unwrap()
        .with_skills(
            skill_profile::Profile::read(serde_json::to_vec(&profile).unwrap().as_slice()).unwrap(),
        )
        .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_learn_new_skill_pays_gold_once_and_survives_relogin() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig, minimum_item_logs, minimum_selection, minimum_skills,
        },
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 700).await;
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    let log = minimum_item_logs::Context {
        flags: minimum_item_logs::flags(&assets).unwrap(),
        civil_offset_seconds: 0,
    };
    let gold = || async {
        sqlx::query_scalar::<_, Option<i32>>(
            "SELECT count FROM game_items WHERE owner_id=1 AND collection=101",
        )
        .fetch_optional(&p)
        .await
        .unwrap()
        .flatten()
    };
    let learned = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM game_skills WHERE cid=1 AND skill_utf16=$1 AND rank=1",
        )
        .bind(utf16("skill_new_synthetic"))
        .fetch_one(&p)
        .await
        .unwrap()
    };
    let result = |packet: &vindictus_server::domain::map_partition::OutgoingPacket| {
        assert_eq!(packet.category, 341);
        let Some(Message::LearnNewSkillResult(code)) =
            inventory::decode(Kind::LearnNewSkillResult, &packet.body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("result missing")
        };
        code
    };
    // OpenLog: one CharacterJournal row and its "Login" ledger row.
    let journal =
        vindictus_server::infrastructure::character_initialization_logs::open_character_journal(
            &p, 1,
        )
        .await;
    assert!(journal > 0);
    let opened: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM game_character_journal01 WHERE id=$1 AND character_id=1)+(SELECT count(*) FROM game_character_journal02 WHERE id=$1 AND character_id=1)+(SELECT count(*) FROM game_character_ledger01 WHERE journal_id=$1)+(SELECT count(*) FROM game_character_ledger02 WHERE journal_id=$1)",
    )
    .bind(journal)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(opened, 2);
    let learn = |ids: Vec<i64>| {
        minimum_skills::learn_new(
            &p,
            &owner,
            &assets,
            1,
            "skill_new_synthetic",
            ids,
            log,
            journal,
        )
    };
    // Not selected: no InGame character.
    assert!(learn(vec![900_000_000_000_000_001]).await.is_err());
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    // No gold: Fail_NotEnoughGold and nothing written.
    let before = items(&p).await;
    let packets = learn(vec![900_000_000_000_000_002]).await.unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(result(&packets[0]), 2);
    assert_eq!((items(&p).await, learned().await), (before, 0));
    // Grant gold offline, select again and learn: gold sync, skill list, Succeeced.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_gold", 1000, 9)
            .await
            .unwrap(),
        Ok(1)
    );
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let packets = learn(vec![900_000_000_000_000_003]).await.unwrap();
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [617, 583, 341]
    );
    assert_eq!(result(&packets[2]), 1);
    let Some(Message::SkillList(list)) =
        inventory::decode(Kind::SkillList, &packets[1].body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("skill list missing")
    };
    assert!(
        list.skills
            .unwrap()
            .iter()
            .flatten()
            .any(|b| b.id.as_deref() == Some("skill_new_synthetic") && b.base_rank == 1)
    );
    assert_eq!((gold().await, learned().await), (Some(300), 1));
    let row: i64 =
        sqlx::query_scalar("SELECT row_id FROM game_skills WHERE cid=1 AND skill_utf16=$1")
            .bind(utf16("skill_new_synthetic"))
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(row, 900_000_000_000_000_003);
    // AddSkillLog(GiveSkill, skill, 1, 0) under the login journal.
    let logged: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM game_character_skill_log01 WHERE journal_id=$1 AND event=1 AND skill_utf16=$2 AND skill_level=1)+(SELECT count(*) FROM game_character_skill_log02 WHERE journal_id=$1 AND event=1 AND skill_utf16=$2 AND skill_level=1)",
    )
    .bind(journal)
    .bind(utf16("skill_new_synthetic"))
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(logged, 1);
    // Already learnt: refused before any payment.
    let packets = learn(vec![900_000_000_000_000_004]).await.unwrap();
    assert_eq!(result(&packets[0]), 7);
    assert_eq!((gold().await, learned().await), (Some(300), 1));
    // Relogin keeps the gold and the skill.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!((gold().await, learned().await), (Some(300), 1));
    cleanup_game_pool(a, p, s).await;
}
/// Minimal NPC script projection: one default NPC and one level-1 auto-give storyline.
fn quest_clock() -> vindictus_server::infrastructure::minimum_quest::QuestClock {
    vindictus_server::infrastructure::minimum_quest::QuestClock {
        arithmetic: vindictus_server::domain::character_stats::Arithmetic::FrameworkX64,
        civil_offset_seconds: 0,
    }
}
fn npc_profile() -> vindictus_server::infrastructure::npc_scripts::Profile {
    let leaf = |name: &str, text: &str| json!([name, [], text, [], ""]);
    let default = json!([
        "DefaultPhase",
        [],
        "",
        [
            [
                "ButtonSet",
                [["id", "base_buttonset"]],
                "",
                [[
                    "Button",
                    [["caption", "bye"], ["command", "endtalk"]],
                    "",
                    [],
                    ""
                ]],
                ""
            ],
            [
                "Talk",
                [
                    ["id", "FERGHUS"],
                    ["location", "t01_bs"],
                    ["command", "greeting"]
                ],
                "",
                [leaf("talk", "hi")],
                ""
            ],
            [
                "Talk",
                [
                    ["id", "FERGHUS"],
                    ["location", "t01_bs"],
                    ["command", "repair"]
                ],
                "",
                [[
                    "Do",
                    [],
                    "",
                    [leaf("ask_repair", "price_text"), leaf("do_repair", "")],
                    ""
                ]],
                ""
            ]
        ],
        ""
    ]);
    let story = json!([
        "StoryLine",
        [],
        "",
        [
            [
                "Phase",
                [["id", "p0"]],
                "",
                [
                    leaf("Goal", "find_ferghus"),
                    [
                        "Talk",
                        [["id", "FERGHUS"]],
                        "",
                        [["Do", [], "", [leaf("talk", "x")], ""]],
                        ""
                    ]
                ],
                ""
            ],
            [
                "Phase",
                [["id", "p1"]],
                "",
                [[
                    "Talk",
                    [["id", "FERGHUS"]],
                    "",
                    [[
                        "Do",
                        [],
                        "",
                        [
                            leaf("set_guide", "landmark_bs,GUIDE_MSG"),
                            leaf("set_token", "count,3"),
                            leaf("increase_token", "count,4"),
                            leaf("give_item", "synthetic_gold,25"),
                            leaf("give_quest", "quest_story"),
                            leaf("goto_phase", "p0")
                        ],
                        ""
                    ]],
                    ""
                ]],
                ""
            ]
        ],
        ""
    ]);
    let info = json!({"StoryLineID":"GuidePlay_1_SkillTrain","StoryLineName":null,"XmlPath":"QuestSet01","Npc":null,"Category":"MainStream","Stage":null,"RewardGold":0,"RewardExp":0,"RewardAp":0,"RewardTitle":0,"RewardItem":null,"RewardItemCount":0,"RewardSkill":null,"RewardSkillRank":0,"AutoGiveLevel":1,"ClassRestriction":1023,"AutoProcess":false,"AutoReveal":true,"Season":0,"Episode":null,"Feature":null});
    let export = json!({"format_version":1,"source":"heroes.npc-scripts","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"scripts_sha256":"c".repeat(64),"defaults":1,"storylines":1,"coerced_integers":0,"shops":2,"coerced_shop_integers":0},
    "features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"NewStoryLineDialog","value":null},{"key":"FlexibleMapLoading","value":"1"},{"key":"Buyback","value":"1"},{"key":"BuybackPriceRatio","value":"2"},{"key":"BuybackListCount","value":"10"},{"key":"RepairPriceRate","value":"100"}],
    "defaults":[{"feature":"","path":"Default/Default_A","tree":default}],
    "storylines":[{"info":info,"tree":story}],
    "repair":{"enhance":[[0,0]],"quality":[[2,6]],"enchant":[[0,0,0]],"equip_class":[["SWORD",2]],"equip_item_class":["sword"]},
    "quests":{"quests":[{"ID":"quest_gnoll_watchman","AutoGiveLevel":1,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false},{"ID":"quest_gnoll_shadowfang","AutoGiveLevel":1,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false},{"ID":"quest_story","AutoGiveLevel":0,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false}],
        "goals":[[10010,"quest_gnoll_watchman",0,100]],"constraints":[["quest_gnoll_shadowfang","quest_gnoll_watchman",100]],
        "difficulties":[["quest_gnoll_watchman",2,1,null],["quest_gnoll_shadowfang",2,1,null]],"periods":[]},
    "shops":[
        {"ShopID":"TEST_Shop","Order":1,"ItemClass":"synthetic_cash","Count":2,"PriceItemClass":"synthetic_gold","PriceCount":100,"AppearQuestID":null,"CharacterBind":false,"ID":null,"RestrictionResetTime":null,"RestrictionPeriodDay":0,"RestrictionCount":0,"Feature":null},
        {"ShopID":"TEST_Shop","Order":2,"ItemClass":"synthetic_cash","Count":1,"PriceItemClass":"gold","PriceCount":10,"AppearQuestID":null,"CharacterBind":false,"ID":7,"RestrictionResetTime":null,"RestrictionPeriodDay":0,"RestrictionCount":5,"Feature":null}
    ]});
    vindictus_server::infrastructure::npc_scripts::Profile::read(
        serde_json::to_vec(&export).unwrap().as_slice(),
    )
    .unwrap()
}
#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_story_login_opens_auto_storylines_once_and_lists_npcs() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{character_lobby::QueryConfig, minimum_selection, minimum_story},
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    assert!(
        minimum_story::npc_list(&p, &owner, &assets, 1)
            .await
            .is_err()
    );
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(
        minimum_story::login_openings(&p, &owner, &assets, 1)
            .await
            .unwrap(),
        1
    );
    // A wrong number of entity IDs writes nothing.
    assert!(
        minimum_story::login(&p, &owner, &assets, 1, vec![])
            .await
            .is_err()
    );
    let packets = minimum_story::login(&p, &owner, &assets, 1, vec![910_000_000_000_000_001])
        .await
        .unwrap();
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [598, 507]
    );
    let Some(Message::StoryLines(lines)) =
        inventory::decode(Kind::StoryLines, &packets[0].body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("story lines missing")
    };
    let line = lines.status.unwrap().remove(0).unwrap();
    assert_eq!(
        (
            line.story_line.as_deref(),
            line.phase.as_deref(),
            line.status,
            line.phase_text.as_deref()
        ),
        (
            Some("GuidePlay_1_SkillTrain"),
            Some("p0"),
            1,
            Some("find_ferghus")
        )
    );
    let stored: (i64, i16) =
        sqlx::query_as("SELECT row_id,status FROM game_story_progress WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(stored, (910_000_000_000_000_001, 1));
    let guide: Vec<u8> =
        sqlx::query_scalar("SELECT guide_target_utf16 FROM game_story_guides WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(guide, utf16("landmark_adv_guild"));
    // The NPC list carries the storyline on its NPC; relogin opens nothing again.
    let list = minimum_story::npc_list(&p, &owner, &assets, 1)
        .await
        .unwrap();
    assert_eq!(list.body, packets[1].body);
    let Some(Message::NpcList(Some(buildings))) =
        inventory::decode(Kind::NpcList, &list.body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("npc list missing")
    };
    let npc = buildings[0].as_ref().unwrap().npcs.as_ref().unwrap()[0]
        .clone()
        .unwrap();
    assert_eq!(npc.npc.as_deref(), Some("FERGHUS"));
    assert_eq!(
        npc.story_lines.unwrap()[0]
            .as_ref()
            .unwrap()
            .story_line
            .as_deref(),
        Some("GuidePlay_1_SkillTrain")
    );
    // Default and storyline talks of the selected character.
    let talk = |packets: Vec<vindictus_server::domain::map_partition::OutgoingPacket>| {
        assert_eq!(packets.len(), 1);
        let Some(Message::NpcTalk(Some(content))) =
            inventory::decode(Kind::NpcTalk, &packets[0].body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("npc talk missing")
        };
        content
            .into_iter()
            .map(|e| {
                let e = e.unwrap();
                (e.command.unwrap(), e.arg1.unwrap())
            })
            .collect::<Vec<_>>()
    };
    let mut session = vindictus_server::domain::npc_talk::Session::default();
    let greeting = minimum_story::talk(
        &p,
        &owner,
        &assets,
        1,
        &mut session,
        minimum_story::TalkRequest::TalkTo {
            location: Some("t01_bs"),
            npc: "FERGHUS",
            story_line: None,
            command: Some("greeting"),
        },
        vec![],
        quest_clock(),
    )
    .await
    .unwrap();
    assert_eq!(
        talk(greeting)[..2],
        [
            ("change_npc".to_owned(), "FERGHUS".to_owned()),
            ("talk".to_owned(), "hi".to_owned())
        ]
    );
    let story = minimum_story::talk(
        &p,
        &owner,
        &assets,
        1,
        &mut session,
        minimum_story::TalkRequest::TalkTo {
            location: None,
            npc: "FERGHUS",
            story_line: Some("GuidePlay_1_SkillTrain"),
            command: None,
        },
        vec![],
        quest_clock(),
    )
    .await
    .unwrap();
    assert_eq!(talk(story)[1], ("talk".to_owned(), "x".to_owned()));
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(
        minimum_story::login_openings(&p, &owner, &assets, 1)
            .await
            .unwrap(),
        0
    );
    assert!(
        minimum_story::login(&p, &owner, &assets, 1, vec![])
            .await
            .unwrap()
            .is_empty()
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM game_story_progress")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, 1);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_shop_buy_pays_once_refuses_without_changes_and_survives_relogin() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig, minimum_item_logs, minimum_selection, minimum_shop,
        },
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    let log = minimum_item_logs::Context {
        flags: minimum_item_logs::flags(&assets).unwrap(),
        civil_offset_seconds: 0,
    };
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_gold", 500, 9)
            .await
            .unwrap(),
        Ok(1)
    );
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let result = |packet: &vindictus_server::domain::map_partition::OutgoingPacket| {
        assert_eq!(packet.category, 235);
        let Some(Message::BuyItemResult(r)) =
            inventory::decode(Kind::BuyItemResult, &packet.body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("result missing")
        };
        (
            r.result,
            r.item_count,
            r.price_item_count,
            r.restriction_order,
        )
    };
    let gold = || async {
        sqlx::query_scalar::<_, i32>(
            "SELECT count FROM game_items WHERE owner_id=1 AND collection=101",
        )
        .fetch_one(&p)
        .await
        .unwrap()
    };
    let cash = || async {
        sqlx::query_scalar::<_, Option<i64>>(
            "SELECT sum(count) FROM game_items WHERE owner_id=1 AND item_class_utf16=$1",
        )
        .bind(utf16("synthetic_cash"))
        .fetch_one(&p)
        .await
        .unwrap()
        .unwrap_or(0)
    };
    let before_cash = cash().await;
    // Three units of 2 items at 100 gold each (PriceCalc.Ceiling leaves 100 as is).
    assert_eq!(
        minimum_shop::entity_ids_needed(&assets, "test_shop", 1, 3),
        1
    );
    let packets = minimum_shop::buy(
        &p,
        &owner,
        &assets,
        1,
        "test_shop",
        1,
        3,
        vec![920_000_000_000_000_001],
        log,
    )
    .await
    .unwrap();
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [617, 235]
    );
    assert_eq!(result(&packets[1]), (1, 6, 300, 1));
    assert_eq!((gold().await, cash().await), (200, before_cash + 6));
    // Not enough gold: Failed with the unit values, nothing changes.
    let state = items(&p).await;
    let packets = minimum_shop::buy(
        &p,
        &owner,
        &assets,
        1,
        "TEST_Shop",
        1,
        3,
        vec![920_000_000_000_000_002],
        log,
    )
    .await
    .unwrap();
    assert_eq!(packets.len(), 1);
    assert_eq!(result(&packets[0]), (2, 2, 100, -1));
    assert_eq!(items(&p).await, state);
    // Silent and unmigrated rows: no message, no change.
    assert!(
        minimum_shop::buy(&p, &owner, &assets, 1, "TEST_Shop", 1, 0, vec![], log)
            .await
            .is_err()
    );
    assert!(
        minimum_shop::buy(&p, &owner, &assets, 1, "TEST_Shop", 9, 1, vec![], log)
            .await
            .is_err()
    );
    assert!(
        minimum_shop::buy(&p, &owner, &assets, 1, "TEST_Shop", 2, 1, vec![], log)
            .await
            .is_err()
    );
    assert!(minimum_shop::has_restricted_rows(&assets, "test_shop"));
    assert_eq!(items(&p).await, state);
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!((gold().await, cash().await), (200, before_cash + 6));
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_shop_sell_pays_sell_price_and_buyback_restores_the_sold_items() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig, minimum_item_logs, minimum_selection, minimum_shop,
        },
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let log = minimum_item_logs::Context {
        flags: minimum_item_logs::flags(&assets).unwrap(),
        civil_offset_seconds: 0,
    };
    assert_eq!(
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_gold", 100, 9)
            .await
            .unwrap(),
        Ok(1)
    );
    minimum_selection::select(
        &p,
        &owner,
        &assets,
        0,
        QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    let gold = || async {
        sqlx::query_scalar::<_, i32>(
            "SELECT count FROM game_items WHERE owner_id=1 AND collection=101",
        )
        .fetch_one(&p)
        .await
        .unwrap()
    };
    let (stack, held): (i64, i32) = sqlx::query_as(
        "SELECT id,count FROM game_items WHERE owner_id=1 AND item_class_utf16=$1 AND count>=2 ORDER BY id LIMIT 1",
    )
    .bind(utf16("synthetic_cash"))
    .fetch_one(&p)
    .await
    .unwrap();
    let mut buyback = Vec::new();
    let categories = |packets: &[vindictus_server::domain::map_partition::OutgoingPacket]| {
        packets.iter().map(|p| p.category).collect::<Vec<_>>()
    };
    // Too many: a dialog and no change; the gold item itself cannot be sold.
    let before = items(&p).await;
    let packets = minimum_shop::sell(
        &p,
        &owner,
        &assets,
        1,
        stack,
        held + 1,
        &mut buyback,
        vec![],
        log,
    )
    .await
    .unwrap();
    assert_eq!(categories(&packets), [602]);
    let gold_id: i64 =
        sqlx::query_scalar("SELECT id FROM game_items WHERE owner_id=1 AND collection=101")
            .fetch_one(&p)
            .await
            .unwrap();
    assert!(
        minimum_shop::sell(
            &p,
            &owner,
            &assets,
            1,
            gold_id,
            1,
            &mut buyback,
            vec![],
            log
        )
        .await
        .is_err()
    );
    assert_eq!(items(&p).await, before);
    // Two sold at SellPrice 7: +14 gold, one buyback entry at 7 × BuybackPriceRatio 2.
    let packets = minimum_shop::sell(&p, &owner, &assets, 1, stack, 2, &mut buyback, vec![], log)
        .await
        .unwrap();
    assert_eq!(categories(&packets), [617, 294, 602]);
    assert_eq!(gold().await, 114);
    let count: i32 = sqlx::query_scalar("SELECT count FROM game_items WHERE id=$1")
        .bind(stack)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(count, held - 2);
    let Some(Message::BuybackListResult(Some(list))) =
        inventory::decode(Kind::BuybackListResult, &packets[1].body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("buyback list missing")
    };
    let entry = list[0].clone().unwrap();
    assert_eq!(
        (entry.order, entry.item_count, entry.price_count),
        (0, 2, 14)
    );
    // Buyback costs 7 × 2 × 1 × 2 = 28 gold and returns the two items.
    let packets = minimum_shop::buy_back(
        &p,
        &owner,
        &assets,
        1,
        0,
        1,
        &mut buyback,
        vec![930_000_000_000_000_001],
        log,
    )
    .await
    .unwrap();
    assert_eq!(categories(&packets), [617, 294, 235]);
    assert_eq!(gold().await, 86);
    assert!(buyback.is_empty());
    let total: i64 = sqlx::query_scalar(
        "SELECT sum(count) FROM game_items WHERE owner_id=1 AND item_class_utf16=$1",
    )
    .bind(utf16("synthetic_cash"))
    .fetch_one(&p)
    .await
    .unwrap();
    let before_total: i64 = before
        .iter()
        .filter(|r| r.1 == utf16("synthetic_cash"))
        .map(|r| i64::from(r.2))
        .sum();
    assert_eq!(total, before_total);
    assert!(
        minimum_shop::buy_back(&p, &owner, &assets, 1, 0, 1, &mut buyback, vec![], log)
            .await
            .is_err()
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_repair_pays_the_source_price_through_message_and_npc() {
    use vindictus_server::{
        domain::{character_stats::Arithmetic, repair},
        infrastructure::{
            character_lobby::QueryConfig, minimum_item_logs, minimum_repair, minimum_selection,
            minimum_story,
        },
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let log = minimum_item_logs::Context {
        flags: minimum_item_logs::flags(&assets).unwrap(),
        civil_offset_seconds: 0,
    };
    // Damage the fixture weapon (MaxDurability 200): 150 durability lost.
    let weapon: i64 = sqlx::query_scalar("SELECT min(id) FROM game_equippables")
        .fetch_one(&p)
        .await
        .unwrap();
    sqlx::query("UPDATE game_equippables SET reduced_durability=150 WHERE id=$1")
        .bind(weapon)
        .execute(&p)
        .await
        .unwrap();
    minimum_selection::select(
        &p,
        &owner,
        &assets,
        0,
        QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    // The fixture weapon has Price 0: RepairPrice is 0 and repair needs no gold.
    let tables = repair::RepairTables {
        quality: [(2, 6)].into_iter().collect(),
        rate: 100,
        ..Default::default()
    };
    assert_eq!(repair::repair_price(&tables, 50, 200, 0, None), 0);
    // 20000 lost × 1,000,000 × ItemValue 50 × 3e-10 × 100 / 100, truncated.
    assert_eq!(
        repair::repair_price(&tables, 0, 20000, 1_000_000, None),
        299
    );
    let mut session = vindictus_server::domain::npc_talk::Session::default();
    let packets = minimum_story::talk(
        &p,
        &owner,
        &assets,
        1,
        &mut session,
        minimum_story::TalkRequest::TalkTo {
            location: Some("t01_bs"),
            npc: "FERGHUS",
            story_line: None,
            command: Some("repair"),
        },
        vec![],
        quest_clock(),
    )
    .await
    .unwrap();
    // StartQuery leaves server commands enabled and Talk.Content.Process runs without
    // DoChain, so a leading server command executes at once: Sync, then the talk.
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [617, 508]
    );
    let Some(Message::NpcTalk(Some(content))) =
        inventory::decode(Kind::NpcTalk, &packets[1].body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("talk missing")
    };
    let commands: Vec<_> = content
        .into_iter()
        .map(|e| e.unwrap().command.unwrap())
        .collect();
    assert_eq!(
        &commands[..3],
        ["change_npc", "talk_repair", "do_repair_successful"]
    );
    let reduced: i32 =
        sqlx::query_scalar("SELECT reduced_durability FROM game_equippables WHERE id=$1")
            .bind(weapon)
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(reduced, 0);
    // RepairItemMessage with nothing to repair still answers with the (empty) Sync.
    let packets = minimum_repair::repair_item(
        &p,
        &owner,
        &assets,
        1,
        minimum_repair::Selection {
            item_ids: &[weapon],
            ..Default::default()
        },
        log,
    )
    .await
    .unwrap();
    assert_eq!(packets[0].category, 617);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_story_commands_persist_phase_guide_tokens_and_items_together() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{character_lobby::QueryConfig, minimum_selection, minimum_story},
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    minimum_selection::select(
        &p,
        &owner,
        &assets,
        0,
        QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap();
    minimum_story::login(&p, &owner, &assets, 1, vec![910_000_000_000_000_001])
        .await
        .unwrap();
    sqlx::query("UPDATE game_story_progress SET phase_utf16=$1 WHERE cid=1")
        .bind(utf16("p1"))
        .execute(&p)
        .await
        .unwrap();
    let gold = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(sum(count),0)::BIGINT FROM game_items WHERE owner_id=1 AND item_class_utf16=$1",
        )
        .bind(utf16("synthetic_gold"))
        .fetch_one(&p)
        .await
        .unwrap()
    };
    let before = gold().await;
    let mut session = vindictus_server::domain::npc_talk::Session::default();
    let request = || minimum_story::TalkRequest::TalkTo {
        location: None,
        npc: "FERGHUS",
        story_line: Some("GuidePlay_1_SkillTrain"),
        command: None,
    };
    let ids = || {
        (1..=32)
            .map(|i| 920_000_000_000_000_000 + i)
            .collect::<Vec<i64>>()
    };
    let mut packets = minimum_story::talk(
        &p,
        &owner,
        &assets,
        1,
        &mut session,
        request(),
        ids(),
        quest_clock(),
    )
    .await
    .unwrap();
    while session.last_server_command.is_some() {
        packets.extend(
            minimum_story::talk(
                &p,
                &owner,
                &assets,
                1,
                &mut session,
                minimum_story::TalkRequest::ExecuteServerCommand,
                ids().into_iter().map(|i| i + 100).collect(),
                quest_clock(),
            )
            .await
            .unwrap(),
        );
    }
    let categories: Vec<i32> = packets.iter().map(|p| p.category).collect();
    for kind in [
        Kind::UpdateStoryGuide,
        Kind::StoryVariables,
        Kind::UpdateInventoryInfo,
        Kind::NpcTalk,
        Kind::StoryLines,
        Kind::NpcList,
    ] {
        assert!(
            categories.contains(&kind.category()),
            "{} missing",
            kind.category()
        );
    }
    let phase: Vec<u8> =
        sqlx::query_scalar("SELECT phase_utf16 FROM game_story_progress WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(phase, utf16("p0"));
    let guide: (Vec<u8>, Vec<u8>) = sqlx::query_as(
        "SELECT guide_target_utf16,guide_message_utf16 FROM game_story_guides WHERE cid=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(guide, (utf16("landmark_bs"), utf16("GUIDE_MSG")));
    let token: (Vec<u8>, Vec<u8>, i32) = sqlx::query_as(
        "SELECT story_line_utf16,key_utf16,value FROM game_story_variables WHERE cid=1",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(token, (utf16("GuidePlay_1_SkillTrain"), utf16("count"), 7));
    assert_eq!(gold().await, before + 25);
    // give_quest adds the quest revealed and pushes QuestProgress.
    assert!(categories.contains(&Kind::QuestProgress.category()));
    let given: (i32, i64) = sqlx::query_as(
        "SELECT revealed,last_play_units300 FROM game_quests WHERE owner_id=1 AND quest_utf16=$1",
    )
    .bind(utf16("quest_story"))
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(given, (1, vindictus_server::domain::quest::NEVER_PLAYED));
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_quest_login_gives_first_and_level_quests_once_and_pushes_progress() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{character_lobby::QueryConfig, minimum_quest, minimum_selection},
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(
        minimum_quest::login_openings(&p, &owner, &assets, 1)
            .await
            .unwrap(),
        2
    );
    // A wrong number of entity IDs writes nothing.
    assert!(
        minimum_quest::login(&p, &owner, &assets, 1, vec![], quest_clock())
            .await
            .is_err()
    );
    let ids = vec![930_000_000_000_000_001, 930_000_000_000_000_002];
    let packets = minimum_quest::login(&p, &owner, &assets, 1, ids, quest_clock())
        .await
        .unwrap();
    assert_eq!(packets.len(), 1);
    let decode = |body: &[u8]| {
        let Some(Message::QuestProgress(q)) =
            inventory::decode(Kind::QuestProgress, body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("quest progress missing")
        };
        q
    };
    let q = decode(&packets[0].body);
    let listed: Vec<(String, bool, bool, i32)> = q
        .progress
        .unwrap()
        .into_iter()
        .map(|e| {
            let e = e.unwrap();
            (e.id.unwrap(), e.revealed, e.available, e.max_play_count)
        })
        .collect();
    // The first quest is revealed; shadowfang needs watchman progress 100.
    assert_eq!(
        listed,
        [
            ("quest_gnoll_watchman".to_owned(), true, true, -1),
            ("quest_gnoll_shadowfang".to_owned(), false, false, -1)
        ]
    );
    assert_eq!(q.achieved_goals, Some(vec![]));
    let rows: Vec<i64> =
        sqlx::query_scalar("SELECT row_id FROM game_quests WHERE owner_id=1 ORDER BY row_id")
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(rows, [930_000_000_000_000_001, 930_000_000_000_000_002]);
    // Relogin gives nothing again; QueryQuestProgress pushes the same state.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    assert_eq!(
        minimum_quest::login_openings(&p, &owner, &assets, 1)
            .await
            .unwrap(),
        0
    );
    let again = minimum_quest::login(&p, &owner, &assets, 1, vec![], quest_clock())
        .await
        .unwrap();
    assert_eq!(decode(&again[0].body).progress.unwrap().len(), 2);
    let queried = minimum_quest::query_progress(&p, &owner, &assets, 1, quest_clock())
        .await
        .unwrap();
    assert_eq!(queried[0].body, again[0].body);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_use_answers_town_and_constraint_dialogs_without_changes() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig, minimum_item_logs, minimum_selection, minimum_use,
        },
        protocol::inventory::UseItem,
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let owner = super::initial_inventory::key();
    let fail_of = |packets: Vec<vindictus_server::domain::map_partition::OutgoingPacket>| {
        assert_eq!(packets.len(), 1);
        let Some(Message::ItemFail(fail)) =
            inventory::decode(Kind::ItemFail, &packets[0].body, DEFAULT_MAX_BODY).unwrap()
        else {
            panic!("item fail missing")
        };
        fail
    };
    for (use_in_town, level, expected) in [
        (0, 0, "UseItemFail_NotUseableItem"),
        (1, 10, "ConstraintFail_LowLevel"),
    ] {
        let assets = assets_with(0, use_in_town, level);
        let log = minimum_item_logs::Context {
            flags: minimum_item_logs::flags(&assets).unwrap(),
            civil_offset_seconds: 0,
        };
        sqlx::query("DELETE FROM game_items WHERE item_class_utf16=$1")
            .bind(utf16("synthetic_cash"))
            .execute(&p)
            .await
            .unwrap();
        minimum_give::lab_grant(&p, &assets, Some(1), "synthetic_cash", 3, 9)
            .await
            .unwrap()
            .unwrap();
        minimum_selection::select(
            &p,
            &owner,
            &assets,
            0,
            QueryConfig {
                max_free: 5,
                care_clock: None,
            },
            Arithmetic::FrameworkX64,
        )
        .await
        .unwrap();
        let item: i64 = sqlx::query_scalar("SELECT id FROM game_items WHERE item_class_utf16=$1")
            .bind(utf16("synthetic_cash"))
            .fetch_one(&p)
            .await
            .unwrap();
        let before = items(&p).await;
        let outcome = minimum_use::use_item(
            &p,
            &owner,
            &assets,
            1,
            UseItem {
                item_id: item,
                target_item_id: -1,
            },
            log,
        )
        .await
        .unwrap();
        let minimum_use::Outcome::Applied(packets) = outcome else {
            panic!("dialog missing")
        };
        assert_eq!(
            fail_of(packets).and_then(|f| f.format),
            Some(format!("GameUI_Heroes_{expected}"))
        );
        assert_eq!(items(&p).await, before);
        minimum_selection::disconnect(&p, 1).await.unwrap();
    }
    cleanup_game_pool(a, p, s).await;
}
