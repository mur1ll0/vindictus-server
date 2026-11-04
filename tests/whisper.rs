use serde_json::Value;
use vindictus_server::infrastructure::{
    minimum_whisper as whispers,
    party_hub::{Notice, PartyHub},
};
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    whisper::{self as wire, Kind, Message, Request},
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

#[test]
fn framework_whisper_messages_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-whisper-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 63);
    for (n, case) in cases.iter().enumerate() {
        let kind = match case["kind"].as_str().unwrap() {
            "whisper_filter_list" => Kind::FilterList,
            "update_whisper_filter" => Kind::UpdateFilter,
            "request_whisper" => Kind::Request,
            "whisper" => Kind::Whisper,
            "whisper_fail" => Kind::Fail,
            _ => panic!("unknown fixture type"),
        };
        let body = bytes(&case["body"]);
        let category = case["category"].as_i64().unwrap() as i32;
        if case["mode"] == "registered-typed" {
            assert_eq!(category, kind.category());
        }
        let typed = bytes(&case["typed_body"]);
        let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
        assert_eq!(
            &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
            typed.as_slice(),
            "case {n}"
        );
        if Kind::INGRESS.contains(&kind) {
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                decoded,
                "ingress case {n}"
            );
        }
        let f = &case["fields"];
        match decoded {
            None => assert!(f.is_null()),
            Some(Message::FilterList(d)) => {
                let theirs = &f["Filter"];
                assert_eq!(d.0.is_none(), theirs.is_null(), "case {n}");
                let rows = d.0.unwrap_or_default();
                let theirs = theirs.as_array().map(Vec::as_slice).unwrap_or_default();
                assert_eq!(rows.len(), theirs.len());
                for ((k, v), t) in rows.iter().zip(theirs) {
                    assert_eq!(Some(k.as_str()), t["key"].as_str());
                    assert_eq!(i64::from(*v), t["value"].as_i64().unwrap());
                }
            }
            Some(Message::UpdateFilter(u)) => {
                assert_eq!(
                    i64::from(u.operation_type),
                    f["OperationType"].as_i64().unwrap()
                );
                assert_eq!(u.target_id, text(&f["TargetID"]));
                assert_eq!(i64::from(u.level), f["Level"].as_i64().unwrap());
            }
            Some(Message::Request(r)) => {
                assert_eq!(r.receiver, text(&f["Receiver"]));
                assert_eq!(r.contents, text(&f["Contents"]));
            }
            Some(Message::Whisper(w)) => {
                assert_eq!(w.sender, text(&f["Sender"]));
                assert_eq!(w.contents, text(&f["Contents"]));
            }
            Some(Message::Fail(w)) => {
                assert_eq!(w.to_name, text(&f["ToName"]));
                assert_eq!(w.reason, text(&f["Reason"]));
            }
        }
    }
}

fn request(to: &str, contents: &str) -> Request {
    Request {
        receiver: Some(to.to_owned()),
        contents: Some(contents.to_owned()),
    }
}
fn fail_of(packets: &[vindictus_server::domain::map_partition::OutgoingPacket]) -> wire::Fail {
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].category, Kind::Fail.category());
    match wire::decode(Kind::Fail, &packets[0].body, DEFAULT_MAX_BODY).unwrap() {
        Some(Message::Fail(f)) => f,
        _ => panic!("not a WhisperFail"),
    }
}

#[test]
fn whisper_reaches_an_entered_receiver_and_nothing_returns_to_the_sender() {
    let hub = PartyHub::new(false, false);
    let (a_tx, mut a_rx) = tokio::sync::mpsc::channel(8);
    let (b_tx, mut b_rx) = tokio::sync::mpsc::channel(8);
    hub.register(1, "Alice", a_tx);
    hub.register(2, "Bob", b_tx);
    assert_eq!(hub.name(1).as_deref(), Some("Alice"));
    let out = whispers::route(&hub, "Alice", &request("bob", "olá 日本"), Some(2)).unwrap();
    assert!(out.is_empty());
    let Ok(Notice::Push(packets)) = b_rx.try_recv() else {
        panic!("no whisper for the receiver");
    };
    assert_eq!(packets.len(), 1);
    assert_eq!(packets[0].category, Kind::Whisper.category());
    let Some(Message::Whisper(w)) =
        wire::decode(Kind::Whisper, &packets[0].body, DEFAULT_MAX_BODY).unwrap()
    else {
        panic!("not a WhisperMessage");
    };
    assert_eq!(w.sender.as_deref(), Some("Alice"));
    assert_eq!(w.contents.as_deref(), Some("olá 日本"));
    assert!(a_rx.try_recv().is_err());
}

#[test]
fn whisper_fails_for_an_unknown_or_offline_receiver() {
    let hub = PartyHub::new(false, false);
    let (a_tx, _a_rx) = tokio::sync::mpsc::channel(8);
    hub.register(1, "Alice", a_tx);
    let f = fail_of(&whispers::route(&hub, "Alice", &request("Nobody", "hi"), None).unwrap());
    assert_eq!(f.to_name.as_deref(), Some("Nobody"));
    assert_eq!(f.reason.as_deref(), Some(whispers::INVALID_NAME));
    let f = fail_of(&whispers::route(&hub, "Alice", &request("Carol", "hi"), Some(3)).unwrap());
    assert_eq!(f.to_name.as_deref(), Some("Carol"));
    assert_eq!(f.reason.as_deref(), Some(whispers::NOT_ONLINE));
    // The receiver left: its seat is gone, so it is offline again.
    let (b_tx, _b_rx) = tokio::sync::mpsc::channel(8);
    hub.register(2, "Bob", b_tx);
    hub.unregister(2);
    let f = fail_of(&whispers::route(&hub, "Alice", &request("Bob", "hi"), Some(2)).unwrap());
    assert_eq!(f.reason.as_deref(), Some(whispers::NOT_ONLINE));
    let f = fail_of(&[whispers::fail(None, whispers::ERROR).unwrap()]);
    assert_eq!(f.to_name, None);
    assert_eq!(f.reason.as_deref(), Some(whispers::ERROR));
}
