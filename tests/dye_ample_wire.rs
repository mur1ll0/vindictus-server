use serde_json::Value;
use vindictus_server::{
    domain::dye_ample::{DistortTable, try_parse},
    protocol::{
        DEFAULT_MAX_BODY,
        dye::{self as wire, Kind, Message},
        server_profile::REGISTERED_CLASSES,
    },
};
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/framework-dye-ample.json")).unwrap()
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn int(v: &Value) -> i32 {
    v.as_i64().unwrap() as i32
}
fn expected(kind: Kind, f: &Value, null: bool) -> Option<Message> {
    if null {
        return None;
    }
    Some(match kind {
        Kind::Used => Message::Used {
            color_table: f["ColorTable"].as_str().map(str::to_owned),
            seeds: [
                int(&f["Seed1"]),
                int(&f["Seed2"]),
                int(&f["Seed3"]),
                int(&f["Seed4"]),
            ],
        },
        Kind::Request => Message::Request {
            x: int(&f["X"]),
            y: int(&f["Y"]),
            is_avatar: f["IsAvatar"].as_bool().unwrap(),
            color_value: f["ColorValue"]
                .as_array()
                .map(|v| v.iter().map(int).collect()),
        },
        Kind::Complete => Message::Complete {
            color: int(&f["Color"]),
        },
    })
}
#[test]
fn framework_dye_ample_fields_bytes_envelopes_and_truncations_match_x86_x64() {
    let fixture = fixture();
    assert_eq!(fixture["profiles"].as_array().unwrap().len(), 2);
    for profile in fixture["profiles"].as_array().unwrap() {
        let cases = profile["wire_cases"].as_array().unwrap();
        assert_eq!(cases.len(), 198);
        for (n, case) in cases.iter().enumerate() {
            let kind = match case["kind"].as_str().unwrap() {
                "used" => Kind::Used,
                "request" => Kind::Request,
                "complete" => Kind::Complete,
                _ => panic!("unknown fixture type"),
            };
            let category = case["category"].as_i64().unwrap() as i32;
            let body = bytes(&case["body"]);
            let typed = bytes(&case["typed_body"]);
            let message = expected(kind, &case["fields"], case["is_null"].as_bool().unwrap());
            assert!(
                REGISTERED_CLASSES
                    .iter()
                    .any(|r| r.category_id == kind.category() && r.type_id == kind.type_id())
            );
            assert_eq!(
                wire::decode_ingress(kind, category, &body, DEFAULT_MAX_BODY).unwrap(),
                message,
                "case {n}"
            );
            assert_eq!(
                wire::decode(kind, &typed, DEFAULT_MAX_BODY).unwrap(),
                message,
                "case {n}"
            );
            assert_eq!(
                wire::encode(kind, message.as_ref(), DEFAULT_MAX_BODY)
                    .unwrap()
                    .as_slice(),
                typed.as_slice(),
                "case {n}"
            );
            for cut in 0..body.len() {
                assert!(
                    wire::decode_ingress(kind, category, &body[..cut], DEFAULT_MAX_BODY).is_err(),
                    "case {n}, cut {cut}"
                );
            }
            let mut trailing = body.clone();
            trailing.push(0);
            assert!(wire::decode_ingress(kind, category, &trailing, DEFAULT_MAX_BODY).is_err());
            assert!(wire::decode_ingress(kind, category, &body, body.len() - 1).is_err());
        }
    }
}
#[test]
fn framework_distort_table_and_int_parse_match_the_rust_port() {
    let fixture = fixture();
    for profile in fixture["profiles"].as_array().unwrap() {
        let tables: std::collections::BTreeMap<String, DistortTable> = profile["tables"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, entries)| {
                let entries: Vec<Option<i32>> = entries
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().map(|v| v as i32))
                    .collect();
                let entries: [Option<i32>; 256] = entries.try_into().unwrap();
                (name.clone(), DistortTable::from_entries(&entries).unwrap())
            })
            .collect();
        assert_eq!(tables.len(), 3);
        let cases = profile["undistort"].as_array().unwrap();
        assert_eq!(cases.len(), 1728);
        for (n, case) in cases.iter().enumerate() {
            let table = &tables[case["table"].as_str().unwrap()];
            let seed: Vec<i32> = case["seed"].as_array().unwrap().iter().map(int).collect();
            assert_eq!(
                table.undistort(int(&case["x"]), int(&case["y"]), seed.try_into().unwrap()),
                (int(&case["out_x"]), int(&case["out_y"])),
                "case {n}"
            );
        }
        for case in profile["int_parse"].as_array().unwrap() {
            let ok = case["ok"].as_bool().unwrap();
            assert_eq!(
                try_parse(case["text"].as_str().unwrap()),
                ok.then(|| int(&case["value"])),
                "{case}"
            );
        }
    }
    // The real dye.displace is 256 bytes: the short-file oracle table is -1 past the end.
    let short: Vec<u8> = (0..200).map(|i| 255 - i as u8).collect();
    let entries: [Option<i32>; 256] =
        std::array::from_fn(|i| Some(if i < 200 { 255 - i as i32 } else { -1 }));
    assert_eq!(
        DistortTable::from_bytes(&short),
        DistortTable::from_entries(&entries).unwrap()
    );
}
