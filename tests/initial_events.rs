use serde_json::{Value, json};
use vindictus_server::domain::{
    feature_policy::FeaturePolicy,
    initial_events::{Marble, Pending, Roulette},
    initial_social,
};
fn social(uid: i32, cid: i64) -> initial_social::Loaded {
    let policy = FeaturePolicy::with_recorded_comparisons(
        [
            ("CurrentVer".into(), Some("a".into())),
            ("InGameGuild".into(), Some("on".into())),
        ],
        [("a".into(), "on".into(), true)],
    )
    .unwrap();
    initial_social::Loaded::load(uid, cid, &policy).unwrap()
}
fn roulette(r: &Roulette) -> Value {
    json!({"cid":r.cid(),"uid":r.uid(),"id":r.id(),"version":r.version(),"expiry_ticks":r.expiry_ticks(),"expired":r.initially_expired(),"slots":r.slot_count(),"appearances":r.appearance_count(),"context_present":r.context_matches_parent()})
}
fn marble(m: &Marble) -> Value {
    json!({"cid":m.cid(),"uid":m.uid(),"id":m.id(),"index":m.current_index(),"schedule":m.schedule_id(),"journal":m.journal_id(),"state":m.state(),"chance_face":m.chance_face(),"chance_indices":m.chance_index_count(),"board_present":m.board_present(),"nodes_present":m.node_list_present(),"initialized":m.initialized(),"context_present":m.context_matches_parent()})
}
fn compare(arch: &str) {
    let f: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-events.json")).unwrap();
    let rows = f[arch].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    for name in [
        "RouletteManager",
        "PlayerService.ItemServiceCore.MarbleSystem.MarbleManager",
    ] {
        assert_eq!(f["static_metadata"]["types"][name]["beforefieldinit"], true);
    }
    let mut supported = 0;
    let mut failures = 0;
    let mut excluded = 0;
    for c in rows {
        let s = social(
            c["uid"].as_i64().unwrap().try_into().unwrap(),
            c["cid"].as_i64().unwrap(),
        );
        let pending = Pending::before_journal(&s);
        assert_eq!(roulette(pending.roulette()), c["roulette"]);
        assert_eq!(marble(pending.marble_before_journal()), c["before"]);
        let trace = c["trace"].as_array().unwrap();
        assert_eq!(
            &trace[..4],
            &[
                json!("item-context"),
                json!("item-context"),
                json!("journal-boundary"),
                json!("DisableLogging")
            ]
        );
        assert_eq!(c["contexts"], 2);
        let disabled = c["disabled"].as_bool().unwrap();
        let mode = c["mode"].as_i64().unwrap();
        if disabled {
            assert_eq!(c["allocations"], 0);
            assert_eq!(c["errors"], 0);
            assert_eq!(trace.len(), 4)
        } else if mode == 3 {
            assert_eq!(c["supported"], false);
            assert_eq!(c["errors"], 0);
            excluded += 1;
            continue;
        } else {
            assert_eq!(c["allocations"], 1);
            assert_eq!(trace.last().unwrap(), "dispose-log");
        }
        assert_eq!(c["supported"], true);
        supported += 1;
        let journal = if disabled || mode != 0 {
            if !disabled {
                failures += 1;
                assert_eq!(c["errors"], 1);
            }
            -1
        } else {
            c["journal_result"].as_i64().unwrap()
        };
        let loaded = pending.finish(journal);
        assert_eq!(marble(loaded.marble()), c["marble"]);
        assert!(loaded.contexts_distinct());
        assert!(loaded.shares_parent());
        assert!(loaded.matches_social(&s));
        assert_eq!(c["same_parent"], true);
        assert_eq!(c["distinct_contexts"], true);
    }
    assert!(supported > 0 && failures > 0 && excluded > 0);
}
#[test]
fn initial_events_match_framework_x86() {
    compare("x86");
}
#[test]
fn initial_events_match_framework_x64() {
    compare("x64");
}
#[test]
fn event_contexts_bind_exact_prior_instead_of_equal_identifiers() {
    let first = social(31, 101);
    let second = social(31, 101);
    let a = Pending::before_journal(&first).finish(201);
    let b = Pending::before_journal(&second).finish(-1);
    assert!(a.matches_social(&first));
    assert!(!a.matches_social(&second));
    assert!(b.matches_social(&second));
    assert_eq!(a.marble().journal_id(), 201);
    assert_eq!(b.marble().journal_id(), -1);
}
