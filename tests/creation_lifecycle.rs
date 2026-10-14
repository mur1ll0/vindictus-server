use vindictus_server::domain::creation_lifecycle::CreationCallbacks;
#[test]
fn creation_callbacks_match_original_complete_fail_order_and_exception_guard() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/framework-creation-lifecycle.json")).unwrap();
    for p in f["profiles"].as_array().unwrap() {
        assert_eq!(p["cases"].as_array().unwrap().len(), 124);
        for c in p["cases"].as_array().unwrap() {
            let mut callbacks = CreationCallbacks::default();
            let mut deletes = 0;
            for event in c["sequence"].as_str().unwrap().bytes() {
                let needed = match event {
                    b'C' => callbacks.completed(c["initialized"].as_bool().unwrap()),
                    b'F' => callbacks.failed(),
                    _ => panic!("unsupported oracle event"),
                };
                if needed {
                    deletes += 1
                }
            }
            assert_eq!(c["deletes"], deletes);
            assert_eq!(c["result"], callbacks.result());
        }
    }
}
