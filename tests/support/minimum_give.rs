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
        .map(|(n, c)| json!({"item_utf16":c["item_utf16"],"category_utf16":null,"unique":if n == 1 { cash_unique } else { 0 },"expire_in":null,"price":0,"sell_price":if n == 1 { 7 } else { 0 },"required_level":if n == 1 { cash_level } else { 0 },"class_restriction":if n == 1 { 2047 } else { 0 },"use_in_town":if n == 1 { cash_use_in_town } else { 0 },"indestructible":0,"level_upper_bound":0,"cafe_only":0,"required_skill_utf16":null,"required_skill_rank":0,"required_enhance":0,"required_enchant":0,"quick_slot_max_stack":0,"draft":0}))
        .collect();
    let economy = json!({"format_version":4,"source":"heroesContents.item-economy","alchemist_boxes":[],"manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"class_rows":3},"classes":rows});
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
fn growth() -> vindictus_server::infrastructure::minimum_growth::GrowthContext {
    vindictus_server::infrastructure::minimum_growth::GrowthContext {
        clock: quest_clock(),
        arithmetic: vindictus_server::domain::character_stats::Arithmetic::FrameworkX64,
        config: vindictus_server::infrastructure::character_lobby::QueryConfig {
            max_free: 5,
            care_clock: None,
        },
        journal: 0,
    }
}
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
            ],
            [
                "Phase",
                [["id", "p2"]],
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
                            leaf("notice_title", "410019"),
                            leaf("acquire_title", "410018"),
                            leaf("give_skill", "skill_new_synthetic"),
                            leaf("complete_storyline", ""),
                            leaf("talk", "done")
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
    let info = json!({"StoryLineID":"GuidePlay_1_SkillTrain","StoryLineName":null,"XmlPath":"QuestSet01","Npc":null,"Category":"MainStream","Stage":null,"RewardGold":0,"RewardExp":400,"RewardAp":3,"RewardTitle":0,"RewardItem":null,"RewardItemCount":0,"RewardSkill":null,"RewardSkillRank":0,"AutoGiveLevel":1,"ClassRestriction":1023,"AutoProcess":false,"AutoReveal":true,"Season":0,"Episode":null,"Feature":null});
    let export = json!({"format_version":1,"source":"heroes.npc-scripts","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"scripts_sha256":"c".repeat(64),"defaults":1,"storylines":1,"coerced_integers":0,"shops":2,"coerced_shop_integers":0},
    "features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"NewStoryLineDialog","value":null},{"key":"FlexibleMapLoading","value":"1"},{"key":"Buyback","value":"1"},{"key":"BuybackPriceRatio","value":"2"},{"key":"BuybackListCount","value":"10"},{"key":"RepairPriceRate","value":"100"},{"key":"MaxLevel","value":"90"},{"key":"AccountTitleStatRatio","value":"80"},{"key":"ExpRate_Story","value":"100"},{"key":"APRate_Story","value":"100"},{"key":"GoldRate_Story","value":"100"},{"key":"ExpRate_Combat","value":"100"},{"key":"GoldRate_Combat","value":"100"},{"key":"APRate_Combat","value":"100"},{"key":"MicroPlayFinishTime","value":"30000"}],
    "defaults":[{"feature":"","path":"Default/Default_A","tree":default}],
    "storylines":[{"info":info,"tree":story}],
    "repair":{"enhance":[[0,0]],"quality":[[2,6]],"enchant":[[0,0,0]],"equip_class":[["SWORD",2]],"equip_item_class":["sword"]},
    "quests":{"quests":[{"ID":"quest_gnoll_watchman","AutoGiveLevel":1,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false},{"ID":"quest_gnoll_shadowfang","AutoGiveLevel":1,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false},{"ID":"quest_story","AutoGiveLevel":0,"QuestSet":1,"ServiceCoin":0,"MaxMemberCount":4,"MaxPlayCount":null,"EnableTodayQuest":"Enable","Season3TodayGroup":null,"AutoReveal":false,"Feature":null,"IsSeason2":false}],
        "goals":[[10010,"quest_gnoll_watchman",0,100]],"constraints":[["quest_gnoll_shadowfang","quest_gnoll_watchman",100]],
        "difficulties":[["quest_gnoll_watchman",2,1,null],["quest_gnoll_shadowfang",2,1,null]],"periods":[]},
    "progression":{"class_stats":[[1,27,44,14,23,1020,130,null],[2,34,48,20,27,1048,130,null],[3,42,52,26,31,1075,130,null]],
        "level_exp":[[1,350,null],[2,1148,null],[3,1855,null]],
        "titles":[[410017,1,2047,0,null],[410018,2,2047,5,null],[410019,9,2047,0,null]],"title_goals":[[1,410017],[2,410017]],
        "title_stats":[[410018,"STR",3],[410018,"HP",10]],"title_sharing":[[410017,1,false],[410018,1,false],[410019,9,false]]},
    "mission":{"quests":[["quest_gnoll_watchman",1,30,3,6,false,null,5,0,null,null,4,false,false,null],["quest_gnoll_shadowfang",2,30,3,6,false,null,null,0,null,null,4,false,true,null]],
        "goals":[["quest_gnoll_watchman",10010,0,"kill npc_gnoll watchman",null,null]],
        "sectors":[["quest_gnoll_watchman",1,32,"01j",100,0,0],["quest_gnoll_watchman",2,33,"01_boss",100,0,0],["quest_gnoll_shadowfang",1,34,"01h",100,0,0]],
        "sector_groups":[],"tower_quests":[],
        "entities":[[32,[["classname","trigger_once"],["targetname","t1"]]],[33,[["classname","npc_template_maker"],["TemplateName","npc_gnoll watchman"],["MaxNPCCount","1"],["DropItem","1"]]]],
        "named_props":[],"settings":[],"setting_conditions":[],
        "monsters":[[1,"npc_gnoll","watchman",-1,200,1000,100]],
        "monster_drops":[[1,null,2,"synthetic_gold",100,false,false,false,null]],
        "quest_difficulties":[["quest_gnoll_watchman",2,1,2,null]],
        "drop_classes":[["synthetic_gold",1,2047,null]],
        "prop_tables":[["quest_gnoll_watchman","ruin",null]],
        "prop_items":[[1,"ruin","WOOD","SERVER","synthetic_gold",100,null]],
        "prop_expectations":[["ruin","WOOD",1]],
        "material_groups":[[4,"WOOD"]],
        "effects":[["phoenix_feather","PlayerRevived","TargetDead",1,["ReviveByClient 50",null,null,"GetStatusByClient 1 revive_invincible_normal",null]]],
        "goal_infos":[[10010,1,1,1,100,0,3,1,1,0,0,"",0,"",1,100]],
        "difficulty_rewards":[["quest_gnoll_watchman",2,10000,100,1,null]],
        "quest_rewards":[["quest_gnoll_watchman",0,true],["quest_gnoll_shadowfang",0,false]]},
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
        growth(),
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
        growth(),
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
        growth(),
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
        growth(),
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
                growth(),
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

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_in_game_enter_pushes_daily_quest_progress_story_lines_and_guide() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig, minimum_quest, minimum_selection, minimum_story,
        },
        protocol::inventory::Kind,
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let config = QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    minimum_selection::select(&p, &owner, &assets, 0, config, Arithmetic::FrameworkX64)
        .await
        .unwrap();
    minimum_story::login(&p, &owner, &assets, 1, vec![910_000_000_000_000_001])
        .await
        .unwrap();
    let ids = vec![930_000_000_000_000_001, 930_000_000_000_000_002];
    minimum_quest::login(&p, &owner, &assets, 1, ids, quest_clock())
        .await
        .unwrap();
    // Never-played quests (1990) differ from today: DailyQuestProcess saves and pushes.
    let daily = minimum_quest::daily_process(&p, &owner, &assets, 1, quest_clock())
        .await
        .unwrap();
    assert_eq!(
        daily.iter().map(|p| p.category).collect::<Vec<_>>(),
        [Kind::QuestProgress.category()]
    );
    // SyncStoryProgress (NewStoryLineDialog off) and SyncGuideInfo.
    let story = minimum_story::in_game_sync(&p, &owner, &assets, 1)
        .await
        .unwrap();
    assert_eq!(
        story.iter().map(|p| p.category).collect::<Vec<_>>(),
        [
            Kind::StoryLines.category(),
            Kind::NpcList.category(),
            Kind::UpdateStoryGuide.category()
        ]
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_storyline_status_is_saved_and_resyncs_the_watching_lines() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{character_lobby::QueryConfig, minimum_selection, minimum_story},
        protocol::inventory::Kind,
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0)
        .await
        .with_npc_scripts(npc_profile())
        .unwrap();
    let owner = super::initial_inventory::key();
    let config = QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    minimum_selection::select(&p, &owner, &assets, 0, config, Arithmetic::FrameworkX64)
        .await
        .unwrap();
    minimum_story::login(&p, &owner, &assets, 1, vec![910_000_000_000_000_001])
        .await
        .unwrap();
    let packets =
        minimum_story::set_story_line_status(&p, &owner, &assets, 1, "GuidePlay_1_SkillTrain", 1)
            .await
            .unwrap();
    // SyncWatchingStoryLines: one StoryLinesMessage.
    assert_eq!(
        packets.iter().map(|p| p.category).collect::<Vec<_>>(),
        [Kind::StoryLines.category()]
    );
    let status: i16 = sqlx::query_scalar("SELECT status FROM game_story_progress WHERE cid=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(status, 1);
    // A storyline the character does not own answers nothing and changes nothing.
    assert!(
        minimum_story::set_story_line_status(&p, &owner, &assets, 1, "missing", 0)
            .await
            .unwrap()
            .is_empty()
    );
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_story_exp_reward_levels_up_with_titles_quests_stats_and_relogin() {
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
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    minimum_story::login(&p, &owner, &assets, 1, vec![910_000_000_000_000_001])
        .await
        .unwrap();
    sqlx::query("UPDATE game_story_progress SET phase_utf16=$1 WHERE cid=1")
        .bind(utf16("p2"))
        .execute(&p)
        .await
        .unwrap();
    let ap_before: i32 = sqlx::query_scalar("SELECT ap FROM game_character_info WHERE id=1")
        .fetch_one(&p)
        .await
        .unwrap();
    let ids = || {
        (1..=512)
            .map(|i| 940_000_000_000_000_000 + i)
            .collect::<Vec<i64>>()
    };
    let mut session = vindictus_server::domain::npc_talk::Session::default();
    let mut packets = minimum_story::talk(
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
        ids(),
        growth(),
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
                ids().into_iter().map(|i| i + 1000).collect(),
                growth(),
            )
            .await
            .unwrap(),
        );
    }
    let categories: Vec<i32> = packets.iter().map(|p| p.category).collect();
    // LevelUp, SkillList, QuestProgress, UpdateTitle, CharacterCommonInfo, UpdateStat,
    // then GiveReward's CharacterCommonInfo and the AP message.
    for c in [476, 583, 546, 191, 609, 619, 229] {
        assert!(categories.contains(&c), "{c} missing in {categories:?}");
    }
    let level_up = packets.iter().find(|p| p.category == 476).unwrap();
    let Some(vindictus_server::protocol::minimum_map::Message::LevelUp {
        level,
        stat_increased,
    }) = vindictus_server::protocol::minimum_map::decode(
        vindictus_server::protocol::minimum_map::Kind::LevelUp,
        &level_up.body,
        DEFAULT_MAX_BODY,
    )
    .unwrap()
    else {
        panic!("level up missing")
    };
    assert_eq!(level, 2);
    assert!(stat_increased.unwrap().contains(&("STR", 7)));
    let (level, exp, str_, luck, ap): (i32, i64, i32, i32, i32) =
        sqlx::query_as("SELECT level,exp,str,luck,ap FROM game_character_info WHERE id=1")
            .fetch_one(&p)
            .await
            .unwrap();
    // Level 2 class STR 34 plus the acquired 410018's TitleStatInfo STR 3.
    assert_eq!((level, exp, str_, luck), (2, 50, 37, 100));
    let hp: i32 = sqlx::query_scalar("SELECT hp FROM game_character_info WHERE id=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(hp, 1048 + 10);
    assert!(ap >= ap_before);
    let titles: Vec<(i32, bool)> =
        sqlx::query_as("SELECT title_id,acquired FROM game_titles WHERE cid=1 ORDER BY title_id")
            .fetch_all(&p)
            .await
            .unwrap();
    // notice_title opened 410019 (AutoGiveLevel 9, not given by the level up);
    // acquire_title gave and acquired 410018 before the level up, which then skips it.
    assert_eq!(titles, [(410017, false), (410018, true), (410019, false)]);
    // give_skill: GiveSkill rank 1 without paying the price; ReportToClient's
    // SkillCompletionMessage goes after the query.
    let given: Vec<(i32, i16)> =
        sqlx::query_as("SELECT rank,locked FROM game_skills WHERE cid=1 AND skill_utf16=$1")
            .bind(utf16("skill_new_synthetic"))
            .fetch_all(&p)
            .await
            .unwrap();
    assert_eq!(given, [(1, 0)]);
    assert_eq!(categories.last(), Some(&582));
    let acquired_time: Option<i64> = sqlx::query_scalar(
        "SELECT acquired_time_units300 FROM game_titles WHERE cid=1 AND title_id=410018",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert!(acquired_time.is_some());
    let phase: Option<Vec<u8>> =
        sqlx::query_scalar("SELECT phase_utf16 FROM game_story_progress WHERE cid=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(phase, None);
    // Relogin at level 2: the lobby and selection accept the leveled character.
    minimum_selection::disconnect(&p, 1).await.unwrap();
    let selected =
        minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
            .await
            .unwrap();
    assert_eq!(selected.cid, 1);
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_mission_member_info_matches_the_selected_character_and_refuses_effects() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{character_lobby::QueryConfig, minimum_mission, minimum_selection},
        protocol::mission::{self as mission_wire, LaunchShipGranted},
    };
    let (a, p, s) = isolated_game_pool().await;
    minimum_character::setup(&p).await;
    minimum_character::initialize(&p).await.unwrap();
    let assets = skill_assets(&p, 0).await;
    let owner = super::initial_inventory::key();
    let config = || QueryConfig {
        max_free: 5,
        care_clock: None,
    };
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let skills: Vec<(Vec<u8>, i32)> =
        sqlx::query_as("SELECT skill_utf16,rank FROM game_skills WHERE cid=1 ORDER BY row_id")
            .fetch_all(&p)
            .await
            .unwrap();
    let member = minimum_mission::member_info(
        &p,
        &owner,
        &assets,
        1,
        0,
        12345,
        None,
        &[("SubWeaponMultiplier".to_owned(), "2".to_owned())],
        config(),
        Arithmetic::FrameworkX64,
    )
    .await
    .unwrap()
    .join;
    assert_eq!((member.order, member.tag, member.key), (0, 0, 12345));
    assert_eq!(
        (member.base_class, member.level, member.transform_cool_down),
        (1, 1, -1)
    );
    assert!(member.name.is_some() && member.is_alive && !member.is_assist);
    // Stats.GetDictionary: FinalStat order with the load entries overwritten in place.
    let stats = member.stats.0.clone().unwrap();
    for key in [
        "TOTAL_LOAD",
        "MEDIUM_LOAD",
        "HEAVY_LOAD",
        "LOAD_PENALTY",
        "SKILL_RANK_SUM",
    ] {
        assert_eq!(stats.iter().filter(|(k, _)| k == key).count(), 1, "{key}");
    }
    // SkillManager.ToDictionary in stored order with the stored ranks.
    let listed = member.skill_list.0.clone().unwrap();
    assert_eq!(listed.len(), skills.len());
    for ((id, rank), (raw, stored)) in listed.iter().zip(&skills) {
        assert_eq!((utf16(id), *rank), (raw.clone(), *stored));
    }
    assert_eq!(
        member.battle_inventory.as_ref().unwrap().consumables.0,
        Some(Vec::new())
    );
    // The host's GameJoinMemberInfo encodes inside LaunchShipGranted.
    let message = mission_wire::Message::LaunchShipGranted(Box::new(LaunchShipGranted {
        quest_id: Some("quest_gnoll_scout".to_owned()),
        adult_rule: 0,
        is_practice_mode: 0,
        host_info: Some(member),
    }));
    let body = mission_wire::encode(
        mission_wire::Kind::LaunchShipGranted,
        Some(&message),
        DEFAULT_MAX_BODY,
    )
    .unwrap();
    assert!(
        mission_wire::decode(
            mission_wire::Kind::LaunchShipGranted,
            &body,
            DEFAULT_MAX_BODY
        )
        .unwrap()
        .is_some()
    );
    // A stored status effect is outside the migrated profile: refused, nothing written.
    sqlx::query("INSERT INTO game_status_effects(row_id,cid,type_utf16,level,remain_time) VALUES(1,1,$1,1,60)")
        .bind(utf16("buff_synthetic"))
        .execute(&p)
        .await
        .unwrap();
    let before = items(&p).await;
    assert!(
        minimum_mission::member_info(
            &p,
            &owner,
            &assets,
            1,
            0,
            1,
            None,
            &[],
            config(),
            Arithmetic::FrameworkX64,
        )
        .await
        .is_err()
    );
    assert_eq!(items(&p).await, before);
    minimum_selection::disconnect(&p, 1).await.unwrap();
    cleanup_game_pool(a, p, s).await;
}

#[tokio::test]
#[ignore = "requires disposable PostgreSQL"]
async fn minimum_mission_accept_launches_the_ship_and_grants_the_sector_sequence() {
    use vindictus_server::{
        domain::character_stats::Arithmetic,
        infrastructure::{
            character_lobby::QueryConfig,
            minimum_growth,
            minimum_mission::{self, Accept, OsMissionRandom, RelayTicket},
            minimum_mission_finish, minimum_quest, minimum_selection,
        },
        protocol::mission::{
            self as mission_wire, AcceptQuest, GameInfo, Kind as MissionKind,
            Message as MissionMessage, ShipOption,
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
    minimum_selection::select(&p, &owner, &assets, 0, config(), Arithmetic::FrameworkX64)
        .await
        .unwrap();
    let option = |difficulty| ShipOption {
        max_member_count: 4,
        swear_member_limit: -1,
        until_force_start: -1,
        min_level: 1,
        max_level: 90,
        is_party_only: 0,
        over18_only: 0,
        difficulty,
        is_season2: false,
        selected_boss_quest_ids: None,
        is_practice_mode: false,
        user_ds_mode: false,
    };
    let accept = |quest: &str, difficulty| AcceptQuest {
        quest_id: Some(quest.to_owned()),
        title: Some(String::new()),
        swear_id: -1,
        option: Some(option(difficulty)),
    };
    let decode = |packet: &vindictus_server::domain::map_partition::OutgoingPacket| {
        let kind = [
            MissionKind::PartyInfo,
            MissionKind::UpdateShip,
            MissionKind::PayCoinCompleted,
            MissionKind::InsertBlessStoneComplete,
            MissionKind::ConnectionRequest,
            MissionKind::LaunchShipGranted,
            MissionKind::GameStartGranted,
        ]
        .into_iter()
        .find(|k| k.category() == packet.category);
        kind.map(|k| {
            mission_wire::decode(k, &packet.body, 1 << 22)
                .unwrap()
                .unwrap()
        })
    };
    // QuestOwner.CheckConstraint needs the quest owned: the quest login gives it.
    minimum_quest::login(
        &p,
        &owner,
        &assets,
        1,
        vec![940_000_000_000_000_001, 940_000_000_000_000_002],
        quest_clock(),
    )
    .await
    .unwrap();
    // A difficulty without QuestDifficultyInfo is refused before the ship exists.
    let refused = minimum_mission::accept_quest(
        &p,
        &owner,
        &assets,
        1,
        &accept("quest_gnoll_watchman", 9),
        |_| panic!("relay must not be reached"),
        config(),
        quest_clock(),
    )
    .await
    .unwrap();
    let Accept::Failed(packets) = refused else {
        panic!("refusal expected")
    };
    assert_eq!(packets.len(), 2);
    assert!(matches!(
        decode(&packets[0]),
        Some(MissionMessage::PartyInfo(_))
    ));
    assert_eq!(packets[1].category, 602);
    // DisableUserShip is refused by QueryQuestDigest's CheckDisableUserShip.
    let refused = minimum_mission::accept_quest(
        &p,
        &owner,
        &assets,
        1,
        &accept("quest_gnoll_shadowfang", 2),
        |_| panic!("relay must not be reached"),
        config(),
        quest_clock(),
    )
    .await
    .unwrap();
    assert!(matches!(refused, Accept::Failed(_)));
    let mut seen_party = 0;
    let outcome = minimum_mission::accept_quest(
        &p,
        &owner,
        &assets,
        1,
        &accept("quest_gnoll_watchman", 2),
        |party| {
            seen_party = party;
            RelayTicket {
                address: "127.0.0.1:27016".parse().unwrap(),
                group: 7,
                sn: 3,
                key: 99,
            }
        },
        config(),
        quest_clock(),
    )
    .await
    .unwrap();
    let Accept::Started {
        mut mission,
        packets,
    } = outcome
    else {
        panic!("accepted expected")
    };
    let messages: Vec<_> = packets.iter().map(|p| decode(p).unwrap()).collect();
    // JoinParty, RegisterShipInfo (+UpdateShip), AddPlayer broadcasts, then the frontend's
    // ConnectionRequest and LaunchShipGranted.
    assert_eq!(messages.len(), 7);
    let MissionMessage::PartyInfo(town) = &messages[0] else {
        panic!()
    };
    assert_eq!(
        (town.party_id, town.party_size, town.state),
        (seen_party, 4, 0)
    );
    let member = town.members.as_ref().unwrap()[0].clone().unwrap();
    assert_eq!((member.slot, member.state, member.level), (0, 3, 1));
    let MissionMessage::PartyInfo(ship) = &messages[1] else {
        panic!()
    };
    assert_eq!((ship.party_size, ship.state), (4, 1));
    let MissionMessage::UpdateShip(update) = &messages[2] else {
        panic!()
    };
    let info = update.info.as_ref().unwrap();
    assert_eq!(
        (
            info.quest_id.as_deref(),
            info.difficulty,
            info.swear_id,
            info.state
        ),
        (Some("quest_gnoll_watchman"), 2, -1, 1)
    );
    assert!(matches!(messages[3], MissionMessage::PayCoinCompleted(_)));
    let MissionMessage::InsertBlessStoneComplete(stones) = &messages[4] else {
        panic!()
    };
    assert_eq!(stones.types, Some(vec![0, 2, 1, 3]));
    let MissionMessage::ConnectionRequest(request) = &messages[5] else {
        panic!()
    };
    assert_eq!(
        (
            request.address.as_deref(),
            request.port,
            request.posix_time,
            request.key,
            request.category.as_deref()
        ),
        (
            Some("127.0.0.1"),
            27016,
            3,
            99,
            Some("MicroPlayServiceCore.MicroPlayService")
        )
    );
    let MissionMessage::LaunchShipGranted(launch) = &messages[6] else {
        panic!()
    };
    assert_eq!(launch.quest_id.as_deref(), Some("quest_gnoll_watchman"));
    assert_eq!(launch.host_info.as_ref().unwrap().order, 0);
    assert_eq!(mission.group, 7);
    // StartGame before RegisterServer is refused (GameStarting needs InShip).
    let early = minimum_mission::start_game(
        &mut mission,
        assets.mission().unwrap(),
        &mut OsMissionRandom,
    )
    .unwrap();
    assert_eq!(early.len(), 1);
    assert_eq!(early[0].category, 602);
    let launched = minimum_mission::register_server(
        &mut mission,
        Some(GameInfo {
            name: Some("host".to_owned()),
            map: Some("01j".to_owned()),
            game_dir: None,
            description: None,
            host_id: 1,
            ds_ip: None,
            ds_port: 0,
        }),
    )
    .unwrap();
    let MissionMessage::PartyInfo(in_ship) = decode(&launched[0]).unwrap() else {
        panic!()
    };
    assert_eq!(in_ship.state, 3);
    let started = minimum_mission::start_game(
        &mut mission,
        assets.mission().unwrap(),
        &mut OsMissionRandom,
    )
    .unwrap();
    let started: Vec<_> = started.iter().map(|p| decode(p).unwrap()).collect();
    // Starting broadcast, GameStartGranted, InGame broadcast.
    assert_eq!(started.len(), 5);
    let MissionMessage::GameStartGranted(granted) = &started[2] else {
        panic!()
    };
    assert_eq!(
        (
            granted.quest_level,
            granted.quest_time,
            granted.difficulty,
            granted.sector_move_game_time
        ),
        (1, 30, 2, 5)
    );
    let sectors = granted.sectors.as_ref().unwrap().sectors.as_ref().unwrap();
    assert_eq!(sectors.len(), 2);
    let boss = sectors[1].as_ref().unwrap();
    assert!(
        boss.entities
            .as_ref()
            .unwrap()
            .iter()
            .any(|e| e.as_deref().is_some_and(|e| e.contains("/Target")))
    );
    let MissionMessage::PartyInfo(in_game) = &started[3] else {
        panic!()
    };
    assert_eq!(in_game.state, 5);
    // MoveToNextSector: the starting sector at InitGameTime (-1), then the boss sector
    // by the base connection, advancing the dungeon hour by SectorMoveGameTime.
    let contents = assets.mission().unwrap();
    assert!(minimum_mission::move_sector(&mut mission, contents, "", 1).unwrap());
    assert_eq!((mission.game_time, mission.in_game), (-1, true));
    assert!(minimum_mission::move_sector(&mut mission, contents, "exit", 2).unwrap());
    assert_eq!(mission.game_time, 4);
    assert!(!minimum_mission::move_sector(&mut mission, contents, "", -1).unwrap());
    // The boss (goal target) dies once: monster-kill EXP and the tracker event.
    let boss = sectors[1]
        .as_ref()
        .unwrap()
        .entities
        .as_ref()
        .unwrap()
        .iter()
        .flatten()
        .find_map(|e| {
            let marker = "\"NPCTargetname\" \"";
            let rest = &e[e.find(marker)? + marker.len()..];
            rest[..rest.find('/')?].parse::<i32>().ok()
        })
        .unwrap();
    // The boss (base drop 1 in the entity) gets DropItem 1 + EvilCoreCount 2 = 3 cores.
    let (kill, extra) = minimum_mission::monster_killed(
        &mut mission,
        contents,
        0,
        boss,
        "attack",
        true,
        100,
        &mut OsMissionRandom,
    )
    .unwrap();
    assert!(extra.is_empty());
    assert!(matches!(
        kill,
        vindictus_server::domain::mission::Kill::Killed {
            is_boss: true,
            drop_item: 3,
            ..
        }
    ));
    assert_eq!(mission.monster_kill_exp, 280);
    assert_eq!(
        mission.events,
        [(1, "kill npc_gnoll watchman attack".to_owned())]
    );
    assert_eq!(mission.boss_killed, ["npc_gnoll watchman"]);
    // The kill meets the essential goal: QuestTargetResult with the goal reward and the
    // success scene; the reward waits MicroPlayFinishTime (30 s).
    let policy = assets.npc_policy().unwrap();
    let ctx = minimum_mission_finish::FinishContext {
        contents,
        policy,
        clock: quest_clock(),
    };
    let now = std::time::Instant::now();
    let scene = minimum_mission_finish::drain(&mut mission, ctx, now).unwrap();
    assert!(mission.events.is_empty());
    let Some(MissionMessage::QuestTargetResult(goal)) =
        mission_wire::decode(MissionKind::QuestTargetResult, &scene[0].body, 1 << 20).unwrap()
    else {
        panic!("target result")
    };
    assert_eq!(
        (goal.goal_id, goal.is_goal_success, goal.gold, goal.ap),
        (10010, true, 100, 3)
    );
    assert_eq!(
        scene[1].category,
        MissionKind::QuestSuccessSceneStart.category()
    );
    let due = minimum_mission_finish::next_job(&mission).unwrap();
    assert_eq!(due.duration_since(now).as_secs(), 30);
    // Kicking the boss core gives the dropped class and announces it.
    let key = format!("{boss}/Target");
    let looted =
        minimum_mission::ragdoll_kicked(&mut mission, contents, 0, &key, 1, false).unwrap();
    assert_eq!(looted, "synthetic_gold");
    let gold_before: i32 = sqlx::query_scalar("SELECT COALESCE(SUM(count),0)::int FROM game_items WHERE owner_id=1 AND item_class_utf16=$1")
        .bind(utf16("synthetic_gold"))
        .fetch_one(&p)
        .await
        .unwrap();
    let mut next_id = 950_000_000_000_000_001_i64..;
    let reward = minimum_mission::give_reward(
        &p,
        &owner,
        &assets,
        &mut mission,
        1,
        &looted,
        1,
        1,
        &mut || Ok(next_id.next().unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(reward.len(), 2);
    let Some(MissionMessage::GetItem(got)) =
        mission_wire::decode(MissionKind::GetItem, &reward[1].body, 1 << 20).unwrap()
    else {
        panic!("get item")
    };
    assert_eq!(
        (got.item_class.as_deref(), got.count, got.core_type),
        (Some("synthetic_gold"), 1, 1)
    );
    let gold_after: i32 = sqlx::query_scalar("SELECT COALESCE(SUM(count),0)::int FROM game_items WHERE owner_id=1 AND item_class_utf16=$1")
        .bind(utf16("synthetic_gold"))
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(gold_after, gold_before + 1);
    assert_eq!(
        mission.events.last().map(|(_, e)| e.as_str()),
        Some("get item synthetic_gold 1")
    );
    // A wood prop of the boss sector drops a server erg; picking it gives the item and
    // answers PutErg.
    minimum_mission::sector_props(&mut mission, contents, &[(40, 4)], &mut OsMissionRandom)
        .unwrap();
    let dropped =
        minimum_mission::prop_broken(&mut mission, contents, 40, 0, &mut OsMissionRandom).unwrap();
    let Some(MissionMessage::DropErg(drop)) =
        mission_wire::decode(MissionKind::DropErg, &dropped[0].body, 1 << 20).unwrap()
    else {
        panic!("drop erg")
    };
    let erg = drop.ergs.unwrap()[0].clone().unwrap();
    assert_eq!(
        (drop.broken_prop, erg.erg_type.as_deref(), erg.winner),
        (40, Some("synthetic_gold"), 0)
    );
    assert_eq!(
        mission.events.last().map(|(_, e)| e.as_str()),
        Some("destroyprop WOOD")
    );
    let picked = minimum_mission::pick_erg(&mut mission, contents, erg.erg_id).unwrap();
    assert!(minimum_mission::pick_erg(&mut mission, contents, erg.erg_id).is_none());
    assert_eq!((picked.kind.as_str(), picked.amount), ("synthetic_gold", 1));
    assert!(minimum_mission::player_killed(&mut mission, 0));
    assert!(!minimum_mission::player_killed(&mut mission, 0));
    // A phoenix feather revives the dead host: ApplyMicroPlayEffects reaches the host.
    let revive =
        minimum_mission::apply_effect(&mut mission, contents, 0, 0, "phoenix_feather", 2).unwrap();
    let Some(MissionMessage::ApplyMicroPlayEffects(effects)) =
        mission_wire::decode(MissionKind::ApplyMicroPlayEffects, &revive[0].body, 1 << 20).unwrap()
    else {
        panic!("effects")
    };
    assert_eq!((effects.caster, effects.effects.unwrap().len()), (0, 2));
    assert!(mission.alive);
    // Alive again: the TargetDead effect no longer applies.
    assert!(
        minimum_mission::apply_effect(&mut mission, contents, 0, 0, "phoenix_feather", 2)
            .unwrap()
            .is_empty()
    );
    // No quick-slot consumable is equipped: the use is refused.
    assert!(
        minimum_mission::use_consumable(&mut mission, contents, 0, 100, 0)
            .unwrap()
            .is_none()
    );
    // Before the success scene ends nothing is due; ReturnToTown is ignored.
    assert!(matches!(
        minimum_mission_finish::return_to_town(&mut mission).unwrap(),
        minimum_mission_finish::Return::Ignored
    ));
    let rc = minimum_mission_finish::RewardContext {
        pool: &p,
        owner: &owner,
        assets: &assets,
        growth: minimum_growth::GrowthContext {
            clock: quest_clock(),
            arithmetic: Arithmetic::FrameworkX64,
            config: config(),
            journal: 0,
        },
    };
    let mut ids = (960_000_000_000_000_001_i64..960_000_000_000_000_100)
        .collect::<Vec<_>>()
        .into_iter();
    let early = minimum_mission_finish::run_jobs(&mut mission, ctx, &rc, &mut ids, now)
        .await
        .unwrap();
    assert!(early.is_empty());
    // GiveReward_EssentialCleared → Season1Success: the reserved boss cores, then
    // RewardCharacter and the result screen.
    let finished = minimum_mission_finish::run_jobs(
        &mut mission,
        ctx,
        &rc,
        &mut ids,
        now + std::time::Duration::from_secs(31),
    )
    .await
    .unwrap();
    let finish = finished
        .iter()
        .find(|p| p.category == MissionKind::FinishGame.category())
        .unwrap();
    let Some(MissionMessage::FinishGame(result)) =
        mission_wire::decode(MissionKind::FinishGame, &finish.body, 1 << 20).unwrap()
    else {
        panic!("finish game")
    };
    let summary = result.summary.unwrap();
    // 280 monster EXP + DifficultyBonus 30 % (difficulty 2); goal gold 100 + 30 %.
    assert_eq!(
        (summary.result, summary.exp, summary.gold, summary.ap),
        (0, 364, 130, 3)
    );
    assert_eq!(summary.cleared_goals.0.as_ref().unwrap()[0].0, 10010);
    assert_eq!((summary.last_progress, summary.current_progress), (0, 100));
    assert!(!result.can_restart && result.can_continue_finished_quest);
    let got_items = finished
        .iter()
        .filter(|p| p.category == MissionKind::GetItem.category())
        .count();
    assert_eq!(got_items, 2);
    let (status, clears): (i32, i32) = sqlx::query_as(
        "SELECT status,clear_count FROM game_quests WHERE owner_id=1 AND quest_utf16=$1",
    )
    .bind(utf16("quest_gnoll_watchman"))
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!((status, clears), (100, 1));
    let goal_status: i32 = sqlx::query_scalar(
        "SELECT status FROM game_quest_goal_achievements WHERE owner_id=1 AND goal_id=10010",
    )
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(goal_status, 1 << 2);
    let difficulty: i32 =
        sqlx::query_scalar("SELECT difficulty FROM game_quest_clear_times WHERE owner_id=1")
            .fetch_one(&p)
            .await
            .unwrap();
    assert_eq!(difficulty, 2);
    let exp: i64 = sqlx::query_scalar("SELECT exp FROM game_character_info WHERE id=1")
        .fetch_one(&p)
        .await
        .unwrap();
    assert!(exp > 0);
    // The per-minute job still runs once and, with the play finished, stops itself.
    let minute = minimum_mission_finish::run_jobs(
        &mut mission,
        ctx,
        &rc,
        &mut ids,
        now + std::time::Duration::from_secs(61),
    )
    .await
    .unwrap();
    assert!(minute.is_empty());
    assert!(minimum_mission_finish::next_job(&mission).is_none());
    // ReturnToTown leaves the party.
    let minimum_mission_finish::Return::Town(back) =
        minimum_mission_finish::return_to_town(&mut mission).unwrap()
    else {
        panic!("town expected")
    };
    assert_eq!(back[0].category, MissionKind::Kicked.category());
    let Some(MissionMessage::ReturnFromQuest(order)) =
        mission_wire::decode(MissionKind::ReturnFromQuest, &back[1].body, 1 << 20).unwrap()
    else {
        panic!("return from quest")
    };
    assert_eq!(order.order, 0);
    // A new mission: giving up before the first sector leaves without a reward.
    let ticket = |_| RelayTicket {
        address: "127.0.0.1:27016".parse().unwrap(),
        group: 8,
        sn: 4,
        key: 98,
    };
    let start = |mission: &mut minimum_mission::Mission| {
        minimum_mission::register_server(
            mission,
            Some(GameInfo {
                name: Some("host".to_owned()),
                map: Some("01j".to_owned()),
                game_dir: None,
                description: None,
                host_id: 1,
                ds_ip: None,
                ds_port: 0,
            }),
        )
        .unwrap();
        minimum_mission::start_game(mission, contents, &mut OsMissionRandom).unwrap();
    };
    let Accept::Started { mut mission, .. } = minimum_mission::accept_quest(
        &p,
        &owner,
        &assets,
        1,
        &accept("quest_gnoll_watchman", 2),
        ticket,
        config(),
        quest_clock(),
    )
    .await
    .unwrap() else {
        panic!("accepted expected")
    };
    start(&mut mission);
    let now = std::time::Instant::now();
    minimum_mission_finish::give_up(&mut mission, now);
    let left = minimum_mission_finish::run_jobs(&mut mission, ctx, &rc, &mut ids, now)
        .await
        .unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].category, MissionKind::Kicked.category());
    assert!(mission.completion.left);
    // Giving up inside the dungeon: the personal GiveUpQuest failure with its result.
    let Accept::Started { mut mission, .. } = minimum_mission::accept_quest(
        &p,
        &owner,
        &assets,
        1,
        &accept("quest_gnoll_watchman", 2),
        ticket,
        config(),
        quest_clock(),
    )
    .await
    .unwrap() else {
        panic!("accepted expected")
    };
    start(&mut mission);
    assert!(minimum_mission::move_sector(&mut mission, contents, "", 1).unwrap());
    let now = std::time::Instant::now();
    minimum_mission_finish::give_up(&mut mission, now);
    let failed = minimum_mission_finish::run_jobs(&mut mission, ctx, &rc, &mut ids, now)
        .await
        .unwrap();
    let finish = failed
        .iter()
        .find(|p| p.category == MissionKind::FinishGame.category())
        .unwrap();
    let Some(MissionMessage::FinishGame(result)) =
        mission_wire::decode(MissionKind::FinishGame, &finish.body, 1 << 20).unwrap()
    else {
        panic!("finish game")
    };
    let summary = result.summary.unwrap();
    assert_eq!((summary.result, summary.exp, summary.gold), (3, 0, 0));
    assert!(!result.can_continue_finished_quest);
    assert!(mission.completion.give_up);
    // UpdateQuestStatus counts the failed play as well.
    let clears: i32 = sqlx::query_scalar(
        "SELECT clear_count FROM game_quests WHERE owner_id=1 AND quest_utf16=$1",
    )
    .bind(utf16("quest_gnoll_watchman"))
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(clears, 2);
    assert!(matches!(
        minimum_mission_finish::return_to_town(&mut mission).unwrap(),
        minimum_mission_finish::Return::Town(_)
    ));
    minimum_selection::disconnect(&p, 1).await.unwrap();
    cleanup_game_pool(a, p, s).await;
}
