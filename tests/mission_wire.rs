use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    mission::{self as wire, Kind, Message},
    server_profile::REGISTERED_CLASSES,
};
const KINDS: [(&str, Kind); 38] = [
    ("accept_quest", Kind::AcceptQuest),
    ("party_info", Kind::PartyInfo),
    ("update_ship", Kind::UpdateShip),
    ("connection_request", Kind::ConnectionRequest),
    ("launch_ship_granted", Kind::LaunchShipGranted),
    ("register_server", Kind::RegisterServer),
    ("start_game", Kind::StartGame),
    ("game_start_granted", Kind::GameStartGranted),
    ("member_info", Kind::MemberInfo),
    ("pay_coin_completed", Kind::PayCoinCompleted),
    (
        "insert_bless_stone_complete",
        Kind::InsertBlessStoneComplete,
    ),
    ("move_to_next_sector", Kind::MoveToNextSector),
    ("all_user_join_complete", Kind::AllUserJoinComplete),
    ("monster_killed", Kind::MonsterKilled),
    ("player_killed", Kind::PlayerKilled),
    ("ragdoll_kicked", Kind::RagdollKicked),
    ("drop_gold_core", Kind::DropGoldCore),
    ("get_item", Kind::GetItem),
    ("sector_prop_list", Kind::SectorPropList),
    ("prop_broken", Kind::PropBroken),
    ("drop_erg", Kind::DropErg),
    ("pick_erg", Kind::PickErg),
    ("put_erg", Kind::PutErg),
    ("monster_damage_report", Kind::MonsterDamageReport),
    ("combat_record", Kind::CombatRecord),
    ("use_consumable", Kind::UseConsumable),
    ("player_revived", Kind::PlayerRevived),
    ("apply_micro_play_effects", Kind::ApplyMicroPlayEffects),
    ("micro_play_event", Kind::MicroPlayEvent),
    ("quest_target_result", Kind::QuestTargetResult),
    ("quest_success_scene_start", Kind::QuestSuccessSceneStart),
    ("finish_game", Kind::FinishGame),
    ("return_to_town", Kind::ReturnToTown),
    ("return_from_quest", Kind::ReturnFromQuest),
    ("kicked", Kind::Kicked),
    ("leave_party", Kind::LeaveParty),
    ("query_recommend_ship", Kind::QueryRecommendShip),
    ("ship_list", Kind::ShipList),
];
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
#[test]
fn mission_messages_roundtrip_the_recovered_serializer_bytes() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-mission-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let mut seen = 0;
    for profile in fixture["profiles"].as_array().unwrap() {
        for case in profile["wire_cases"].as_array().unwrap() {
            let name = case["kind"].as_str().unwrap();
            let kind = KINDS.iter().find(|(n, _)| *n == name).unwrap().1;
            assert!(
                REGISTERED_CLASSES
                    .iter()
                    .any(|r| r.category_id == kind.category() && r.type_id == kind.type_id())
            );
            let category = case["category"].as_i64().unwrap() as i32;
            assert_eq!(category, kind.category());
            let body = bytes(&case["body"]);
            let decoded = wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY)
                .unwrap_or_else(|e| panic!("{name} {seen} {e:?}"));
            assert_eq!(decoded.is_none(), case["is_null"].as_bool().unwrap());
            let encoded = wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap();
            assert_eq!(encoded.as_slice(), body.as_slice(), "{name}");
            // Every strict prefix is rejected rather than partially decoded.
            for cut in 0..body.len() {
                assert!(wire::decode(kind, &body[..cut], DEFAULT_MAX_BODY).is_err());
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 278);
}
#[test]
fn accept_quest_and_connection_request_fields_follow_declaration_order() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-mission-wire.json")).unwrap();
    let cases = fixture["profiles"][0]["wire_cases"].as_array().unwrap();
    let body = |kind: &str, i: usize| {
        bytes(&cases.iter().filter(|c| c["kind"] == kind).nth(i).unwrap()["body"])
    };
    let Some(Message::AcceptQuest(a)) = wire::decode(
        Kind::AcceptQuest,
        &body("accept_quest", 3),
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("accept quest")
    };
    assert_eq!(a.quest_id.as_deref(), Some("quest_gnoll_scout"));
    assert_eq!(a.swear_id, 1);
    let option = a.option.unwrap();
    assert_eq!((option.max_member_count, option.difficulty), (1, 2));
    assert_eq!(option.selected_boss_quest_ids, Some(vec![1, i32::MIN]));
    assert!(option.is_practice_mode);
    let Some(Message::ConnectionRequest(c)) = wire::decode(
        Kind::ConnectionRequest,
        &body("connection_request", 2),
        DEFAULT_MAX_BODY,
    )
    .unwrap() else {
        panic!("connection request")
    };
    assert_eq!(c.address.as_deref(), Some("127.0.0.1"));
    // The relay serial number round-trips through ToDateTime().ToInt32() unchanged.
    assert_eq!((c.port, c.posix_time, c.key), (27015, 1, 6));
    assert_eq!((c.ping_host_cid, c.group_id), (-1, 0));
    assert_eq!(
        c.category.as_deref(),
        Some("MicroPlayServiceCore.MicroPlayService")
    );
}
#[test]
fn boat_ship_list_is_empty_and_ignores_a_repeated_target_within_two_seconds() {
    use std::time::{Duration, Instant};
    use vindictus_server::infrastructure::minimum_mission::recommend_ships;
    use vindictus_server::protocol::mission::{QueryRecommendShip, RecommendShipRestriction};
    let query = |set: i32, target: Option<&str>| QueryRecommendShip {
        restriction: Some(RecommendShipRestriction {
            quest_set: set,
            target_quest_id: target.map(str::to_owned),
            difficulty: 0,
            is_season2: false,
            selected_boss_quest_ids: None,
        }),
    };
    let decode = |packets: Vec<vindictus_server::domain::map_partition::OutgoingPacket>| {
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].category, Kind::ShipList.category());
        match wire::decode(Kind::ShipList, &packets[0].body, DEFAULT_MAX_BODY).unwrap() {
            Some(Message::ShipList(list)) => (list.ignored, list.ships),
            _ => panic!("ship list"),
        }
    };
    let start = Instant::now();
    let mut last = None;
    // The whole board (empty target) is answered and starts the cooldown.
    assert_eq!(
        decode(recommend_ships(&query(1, Some("")), &mut last, start).unwrap()),
        (false, Some(Vec::new()))
    );
    assert_eq!(last, Some(start));
    // A targeted query (null counts as targeted, as `!= ""` in the source) within 2 s is Ignored.
    let soon = start + Duration::from_millis(2000);
    assert_eq!(
        decode(recommend_ships(&query(1, None), &mut last, soon).unwrap()),
        (true, Some(Vec::new()))
    );
    assert_eq!(last, Some(start));
    let later = start + Duration::from_millis(2001);
    assert_eq!(
        decode(recommend_ships(&query(1, Some("quest_gnoll_watchman")), &mut last, later).unwrap()),
        (false, Some(Vec::new()))
    );
    assert_eq!(last, Some(later));
    // Quest set 2000 (DS raids) answers an empty list without touching the cooldown.
    let mut untouched = None;
    assert_eq!(
        decode(recommend_ships(&query(2000, Some("")), &mut untouched, later).unwrap()),
        (false, Some(Vec::new()))
    );
    assert_eq!(untouched, None);
    let missing = QueryRecommendShip { restriction: None };
    assert!(recommend_ships(&missing, &mut last, later).is_err());
}
#[test]
fn proudnet_guid_string_is_the_registry_form() {
    use vindictus_server::protocol::proudnet::{PROTOCOL_VERSION, guid_string};
    assert_eq!(
        guid_string(&PROTOCOL_VERSION),
        "0afa3c0c-77d7-4b74-9ddb-1cb3d25e1e63"
    );
}
