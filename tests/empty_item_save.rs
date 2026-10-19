use vindictus_server::application::empty_item_save::{Effect, Flow, after_submit};
#[test]
fn empty_item_save_matches_complete_original_framework_order_and_reload_boundary() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-empty-item-save.json")).unwrap();
    assert_eq!(f["x86"], f["x64"]);
    assert_eq!(f["x86"].as_array().unwrap().len(), 32);
    for c in f["x86"].as_array().unwrap() {
        for k in c["kinds"].as_array().unwrap() {
            let id = k["id"].as_str().unwrap().parse::<i64>().unwrap();
            assert_eq!(
                vindictus_server::application::empty_item_save::is_character_entity(id),
                k["character"].as_bool().unwrap()
            )
        }
        let accepted = c["accepted"].as_bool().unwrap();
        let queued = c["queued"].as_bool().unwrap();
        let mut calls = vec!["submit"];
        if !accepted {
            calls.push("provider-error")
        }
        let flow = after_submit(accepted, queued, |e| {
            calls.push(match e {
                Effect::QueueSync => "enqueue",
                Effect::Sync => "sync",
                Effect::ProcessAfterDelete => "after-delete",
                Effect::WriteLogs => "write",
            })
        });
        let mut expected = c["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap())
            .collect::<Vec<_>>();
        if flow == Flow::ReloadRequired {
            assert_eq!(expected.pop(), Some("undo-required"))
        }
        // The logger's full Write method has no observable writes for an empty list.
        calls.retain(|c| *c != "write");
        assert_eq!(calls, expected);
        assert_eq!(flow == Flow::Saved, c["saved"].as_bool().unwrap());
        assert_eq!(c["evt"], 124);
        assert_eq!(c["journal"], c["jid"]);
        assert_eq!(c["bindings"], true);
        assert_eq!(c["published"], false);
        assert_eq!(c["jobs"].as_u64().unwrap(), u64::from(accepted && queued));
        assert_eq!(
            c["deleted"].as_bool().unwrap(),
            accepted && c["delete"].as_bool().unwrap()
        );
    }
}
