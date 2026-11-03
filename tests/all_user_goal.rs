use serde_json::Value;
use std::collections::BTreeMap;
use vindictus_server::domain::{
    all_user_goal::{Goals, atoms},
    feature_policy::FeaturePolicy,
};
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    all_user_goal::{self as wire, Kind, Message},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}

#[test]
fn framework_all_user_goal_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-all-user-goal-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    for (n, case) in profiles[0]["wire_cases"]
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
    {
        let kind = match case["kind"].as_str().unwrap() {
            "all_user_goal_event" => Kind::Event,
            "all_user_goal_event_modify" => Kind::EventModify,
            "all_user_goal_reward" => Kind::Reward,
            _ => panic!("unknown fixture type"),
        };
        if case["mode"] == "registered-typed" {
            assert_eq!(case["category"].as_i64(), Some(i64::from(kind.category())));
        }
        let typed = bytes(&case["typed_body"]);
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        let f = &case["fields"];
        match decoded {
            None => assert!(f.is_null()),
            Some(Message::EventModify { goal_id, count }) => {
                assert_eq!(i64::from(goal_id), f["GoalID"].as_i64().unwrap());
                assert_eq!(i64::from(count), f["Count"].as_i64().unwrap());
            }
            Some(Message::Reward(goal_id)) => {
                assert_eq!(i64::from(goal_id), f["GoalID"].as_i64().unwrap())
            }
            Some(Message::Event(d)) => {
                let theirs = &f["AllUserGoalInfo"];
                assert_eq!(d.0.is_none(), theirs.is_null(), "case {n}");
                let rows = d.0.unwrap_or_default();
                let theirs = theirs.as_array().map(Vec::as_slice).unwrap_or_default();
                assert_eq!(rows.len(), theirs.len());
                for ((k, v), t) in rows.iter().zip(theirs) {
                    assert_eq!(i64::from(*k), t["key"].as_i64().unwrap());
                    assert_eq!(i64::from(*v), t["value"].as_i64().unwrap());
                }
            }
        }
    }
}

const DAY: i64 = 86_400;
/// 2014-01-15 07:00 UTC, the first row's StartTime.
const T0: i64 = 1_389_769_200;

fn s(v: &str) -> Option<String> {
    Some(v.to_owned())
}
fn goals() -> Goals {
    Goals::load(vec![
        (
            589,
            1,
            s("Event_goal_eneu"),
            1,
            "kill npc_gremlin2 (rant|rant_second)".into(),
            150,
            s("event_box"),
            1,
            T0,
            T0 + DAY,
            s("GOAL_571"),
        ),
        (
            590,
            1,
            s("Event_goal_eneu"),
            1,
            "kill npc_gremlin2 gianoo".into(),
            150,
            s("event_box"),
            1,
            T0 + DAY,
            T0 + 2 * DAY,
            s("GOAL_572"),
        ),
        (
            599,
            2,
            s("Event_goal_eneu"),
            2,
            "kill npc_prop_monster revenger".into(),
            300,
            s("event_box"),
            1,
            T0 + DAY,
            T0 + 3 * DAY,
            s("GOAL_581"),
        ),
        // Another Feature: a separate window group.
        (
            700,
            3,
            s("Event_other"),
            1,
            "kill npc_x".into(),
            10,
            None,
            0,
            T0 + 10 * DAY,
            T0 + 11 * DAY,
            None,
        ),
        (
            800,
            4,
            s("Off"),
            1,
            "kill .*".into(),
            1,
            None,
            0,
            T0,
            T0 + DAY,
            None,
        ),
    ])
    .unwrap()
}
fn policy() -> FeaturePolicy {
    FeaturePolicy::npc_source([
        ("CurrentVer".to_owned(), s("3.09.00")),
        ("Event_goal_eneu".to_owned(), s("1")),
        ("Event_other".to_owned(), s("1")),
        ("Off".to_owned(), None),
    ])
    .unwrap()
}

#[test]
fn turned_on_goals_start_at_the_switch_and_follow_their_windows() {
    let mut g = goals();
    // Published on 2026-11-03 00:00 UTC; Event_other keeps the content's dates.
    let on = 1_793_664_000;
    let since: BTreeMap<String, i64> = [("Event_goal_eneu".to_owned(), on)].into();
    g.refresh(&policy(), &since).unwrap();
    let never = |_: i32| false;
    // The Off row is left out; the group moved by on − T0.
    assert!(g.goal(800).is_none());
    assert_eq!(
        (g.goal(589).unwrap().start, g.goal(589).unwrap().end),
        (on, on + DAY)
    );
    assert_eq!(g.goal(599).unwrap().end, on + 3 * DAY);
    assert_eq!(g.goal(700).unwrap().start, T0 + 10 * DAY);
    // Day 1: the regex is case-insensitive; the other slot-1 goal has not started.
    let now = on + 3_600;
    assert_eq!(
        g.current_id_by_goal("kill NPC_GREMLIN2 rant_second", now, &never),
        589
    );
    assert_eq!(
        g.current_id_by_goal("kill npc_gremlin2 gianoo", now, &never),
        -1
    );
    assert_eq!(
        g.current_id_by_goal("kill npc_gremlin2 rant", now, &|id| id == 589),
        -1
    );
    assert_eq!(g.current_by_slot(1, now).map(|x| x.id), Some(589));
    assert_eq!(g.next_by_slot(1, now).map(|x| x.id), Some(590));
    assert!(g.current_by_slot(2, now).is_none());
    assert_eq!(g.next_refresh(now), Some(on + DAY));
    // The day-1 boundary: slot 1's goal ends as the next starts (no −1), slots 1 and 2 start.
    assert_eq!(g.schedule(now), [(on + DAY, 590, 0), (on + DAY, 599, 0)]);
    // Day 3: slot 1 has no next goal (−2) at its end, slot 2 ends later; the next start is
    // Event_other's (content dates, already past), so nothing else.
    let late = on + 2 * DAY + 60;
    assert_eq!(g.schedule(late), [(on + 3 * DAY, 599, -2)]);
    // Turning the switch on again later moves the group again from the content's times.
    let since: BTreeMap<String, i64> = [("Event_goal_eneu".to_owned(), on + 100)].into();
    g.refresh(&policy(), &since).unwrap();
    assert_eq!(g.goal(589).unwrap().start, on + 100);
    assert_eq!(atoms("Event_a&&!koKR || enEU"), ["Event_a", "koKR", "enEU"]);
    assert!(Goals::is_group(1) && !Goals::is_group(2));
}
