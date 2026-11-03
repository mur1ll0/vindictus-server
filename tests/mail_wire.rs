use serde_json::Value;
use vindictus_server::protocol::{
    DEFAULT_MAX_BODY,
    mail::{self as wire, BriefMailInfo, Kind, MailItemInfo, Message},
    server_profile::REGISTERED_CLASSES,
};

fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn kind(name: &str) -> Kind {
    match name {
        "query_mail_list" => Kind::QueryMailList,
        "query_mail_info" => Kind::QueryMailInfo,
        "send_mail" => Kind::SendMail,
        "get_mail_item" => Kind::GetMailItem,
        "delete_mail" => Kind::DeleteMail,
        "return_mail" => Kind::ReturnMail,
        "mail_list" => Kind::MailList,
        "mail_info" => Kind::MailInfo,
        "mail_sent" => Kind::MailSent,
        "mail_received" => Kind::MailReceived,
        "get_mail_item_completed" => Kind::GetMailItemCompleted,
        _ => panic!("unknown fixture type"),
    }
}
fn int(v: &Value) -> i64 {
    v.as_i64().unwrap()
}
fn item(ours: Option<&MailItemInfo>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(i) = ours else { return };
    for (ours, key) in [
        (i.mail_item_id, "MailItemID"),
        (i.item_id, "ItemID"),
        (i.expire_date_time_diff, "ExpireDateTimeDiff"),
        (i.expire_date_time_tick, "ExpireDateTimeTick"),
        (i64::from(i.item_count), "ItemCount"),
        (i64::from(i.color1), "Color1"),
        (i64::from(i.color2), "Color2"),
        (i64::from(i.color3), "Color3"),
        (i64::from(i.max_durability_bonus), "MaxDurabilityBonus"),
        (i64::from(i.reduced_durability), "ReducedDurability"),
    ] {
        assert_eq!(ours, int(&theirs[key]), "case {n} {key}");
    }
    assert_eq!(
        i.item_class_ex.as_deref(),
        theirs["ItemClassEx"].as_str(),
        "case {n}"
    );
    for (ours, key) in [
        (i.received, "Received"),
        (i.is_character_binded, "IsCharacterBinded"),
        (i.is_expireable, "IsExpireable"),
    ] {
        assert_eq!(ours, theirs[key].as_bool().unwrap(), "case {n} {key}");
    }
}
fn items(ours: &Option<Vec<Option<MailItemInfo>>>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(list) = ours else { return };
    let theirs = theirs.as_array().unwrap();
    assert_eq!(list.len(), theirs.len(), "case {n}");
    for (o, t) in list.iter().zip(theirs) {
        item(o.as_ref(), t, n);
    }
}
fn brief(ours: Option<&BriefMailInfo>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(b) = ours else { return };
    assert_eq!(b.mail_id, int(&theirs["MailID"]), "case {n}");
    assert_eq!(i64::from(b.mail_type), int(&theirs["MailType"]), "case {n}");
    assert_eq!(
        b.from_name.as_deref(),
        theirs["FromName"].as_str(),
        "case {n}"
    );
    assert_eq!(b.to_name.as_deref(), theirs["ToName"].as_str(), "case {n}");
    assert_eq!(b.title.as_deref(), theirs["Title"].as_str(), "case {n}");
    assert_eq!(
        i64::from(b.expire_date),
        int(&theirs["ExpireDate"]),
        "case {n}"
    );
    assert_eq!(b.is_read, theirs["IsRead"].as_bool().unwrap(), "case {n}");
    assert_eq!(
        i64::from(b.item_attached),
        int(&theirs["ItemAttached"]),
        "case {n}"
    );
    assert_eq!(
        i64::from(b.charged_gold),
        int(&theirs["ChargedGold"]),
        "case {n}"
    );
    items(&b.attached_item_info, &theirs["AttachedItemInfo"], n);
}
fn briefs(ours: &Option<Vec<Option<BriefMailInfo>>>, theirs: &Value, n: usize) {
    assert_eq!(ours.is_none(), theirs.is_null(), "case {n}");
    let Some(list) = ours else { return };
    let theirs = theirs.as_array().unwrap();
    assert_eq!(list.len(), theirs.len(), "case {n}");
    for (o, t) in list.iter().zip(theirs) {
        brief(o.as_ref(), t, n);
    }
}

fn check(case: &Value, n: usize) {
    let kind = kind(case["kind"].as_str().unwrap());
    let body = bytes(&case["body"]);
    let typed = bytes(&case["typed_body"]);
    let category = case["category"].as_i64().unwrap() as i32;
    let ingress = wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap();
    let decoded = wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap();
    assert_eq!(ingress, decoded, "case {n}");
    assert_eq!(
        &*wire::encode(kind, decoded.as_ref(), DEFAULT_MAX_BODY).unwrap(),
        typed.as_slice(),
        "case {n}"
    );
    assert_eq!(decoded.is_none(), case["is_null"].as_bool().unwrap());
    let f = &case["fields"];
    match &decoded {
        None => assert!(f.is_null()),
        Some(Message::QueryMailList) => {}
        Some(Message::QueryMailInfo(id))
        | Some(Message::GetMailItem(id))
        | Some(Message::ReturnMail(id)) => assert_eq!(*id, int(&f["MailID"]), "case {n}"),
        Some(Message::DeleteMail(list)) => {
            let theirs = &f["MailList"];
            assert_eq!(list.is_none(), theirs.is_null(), "case {n}");
            if let Some(list) = list {
                let theirs: Vec<i64> = theirs.as_array().unwrap().iter().map(int).collect();
                assert_eq!(list, &theirs, "case {n}");
            }
        }
        Some(Message::SendMail(m)) => {
            assert_eq!(i64::from(m.mail_type), int(&f["MailType"]), "case {n}");
            assert_eq!(m.to_name.as_deref(), f["ToName"].as_str(), "case {n}");
            assert_eq!(m.title.as_deref(), f["Title"].as_str(), "case {n}");
            assert_eq!(m.content.as_deref(), f["Content"].as_str(), "case {n}");
            assert_eq!(i64::from(m.gold), int(&f["Gold"]), "case {n}");
            assert_eq!(m.express, f["Express"].as_bool().unwrap(), "case {n}");
            assert_eq!(
                i64::from(m.charged_gold),
                int(&f["ChargedGold"]),
                "case {n}"
            );
            let theirs = &f["ItemList"];
            assert_eq!(m.item_list.is_none(), theirs.is_null(), "case {n}");
            if let Some(list) = &m.item_list {
                let theirs = theirs.as_array().unwrap();
                assert_eq!(list.len(), theirs.len(), "case {n}");
                for (o, t) in list.iter().zip(theirs) {
                    assert_eq!(o.is_none(), t.is_null(), "case {n}");
                    if let Some(o) = o {
                        assert_eq!(o.item_id, int(&t["ItemID"]), "case {n}");
                        assert_eq!(o.item_class.as_deref(), t["ItemClass"].as_str(), "case {n}");
                        assert_eq!(i64::from(o.item_count), int(&t["ItemCount"]), "case {n}");
                    }
                }
            }
        }
        Some(Message::MailList(m)) => {
            briefs(&m.received_mail_list, &f["ReceivedMailList"], n);
            briefs(&m.sent_mail_list, &f["SentMailList"], n);
            assert_eq!(
                i64::from(m.new_mail_count),
                int(&f["NewMailCount"]),
                "case {n}"
            );
        }
        Some(Message::MailInfo(m)) => {
            assert_eq!(m.mail_id, int(&f["MailID"]), "case {n}");
            items(&m.items, &f["Items"], n);
            assert_eq!(i64::from(m.mail_type), int(&f["MailType"]), "case {n}");
            assert_eq!(m.from_name.as_deref(), f["FromName"].as_str(), "case {n}");
            assert_eq!(m.title.as_deref(), f["Title"].as_str(), "case {n}");
            assert_eq!(m.content.as_deref(), f["Content"].as_str(), "case {n}");
            assert_eq!(
                i64::from(m.charged_gold),
                int(&f["ChargedGold"]),
                "case {n}"
            );
        }
        Some(Message::MailSent(code)) => {
            assert_eq!(i64::from(*code), int(&f["ErrorCode"]), "case {n}")
        }
        Some(Message::MailReceived(m)) => {
            assert_eq!(m.from_name.as_deref(), f["FromName"].as_str(), "case {n}");
            assert_eq!(i64::from(m.mail_type), int(&f["MailType"]), "case {n}");
        }
        Some(Message::GetMailItemCompleted(m)) => {
            assert_eq!(m.mail_id, int(&f["MailID"]), "case {n}");
            assert_eq!(i64::from(m.result), int(&f["Result"]), "case {n}");
        }
    }
}

#[test]
fn framework_mail_envelopes_match_x86_x64() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-mail-wire.json")).unwrap();
    assert_eq!(fixture["original_services_executed"], false);
    let profiles = fixture["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0]["wire_cases"], profiles[1]["wire_cases"]);
    for kind in [
        Kind::QueryMailList,
        Kind::QueryMailInfo,
        Kind::SendMail,
        Kind::GetMailItem,
        Kind::DeleteMail,
        Kind::ReturnMail,
        Kind::MailList,
        Kind::MailInfo,
        Kind::MailSent,
        Kind::MailReceived,
        Kind::GetMailItemCompleted,
    ] {
        assert!(
            REGISTERED_CLASSES
                .iter()
                .any(|r| r.category_id == kind.category() && r.type_id == kind.type_id())
        );
    }
    let cases = profiles[0]["wire_cases"].as_array().unwrap();
    assert_eq!(cases.len(), 120);
    for (n, case) in cases.iter().enumerate() {
        check(case, n);
    }
}

#[test]
fn send_fee_terms_match_the_recovered_expression() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-mail-wire.json")).unwrap();
    for profile in fixture["profiles"].as_array().unwrap() {
        let cases = profile["fee_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 9);
        for c in cases {
            let (price, rate, count) = (
                int(&c["price"]) as i32,
                int(&c["rate"]) as i32,
                int(&c["count"]) as i32,
            );
            assert_eq!(
                i64::from(vindictus_server::domain::mail::send_fee(
                    0,
                    rate,
                    [(price, count)]
                )),
                int(&c["term"]),
                "{} {price} {rate} {count}",
                profile["architecture"]
            );
        }
    }
}
