use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use vindictus_server::{
    application::lobby_costume,
    domain::character_costume::{Costume, CostumeSet},
    protocol::character_list::Costume as Info,
};
#[derive(Deserialize, Serialize)]
struct Row {
    is_base: bool,
    kind: u8,
    key: u8,
    serial: Option<i32>,
    colors: [Option<i32>; 3],
    gloss: Option<u8>,
    effect: Option<i32>,
}
#[derive(Deserialize)]
struct Case {
    n: i32,
    input_sha256: String,
    expected_sha256: String,
}
#[derive(Deserialize)]
struct Profile {
    profile: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Fixtures {
    profiles: Vec<Profile>,
}
fn snapshot(i: Info) -> Value {
    json!({"dictionaries":[i.costume_type_info,i.color_info,i.avatar_info,i.avatar_hide_info,i.pollution_info,i.effect_info,i.decoration_info,i.decoration_color_info,i.body_shape_info],"scalars":[i.shineness,i.height,i.bust,i.painting_pos_x,i.painting_pos_y,i.painting_rotation,i.painting_size,i32::from(i.hide_head_costume),i.cafe_type,i32::from(i.is_return),i.vip_code,i.body_painting_pos_x,i.body_painting_pos_y,i.body_painting_rotation,i.body_painting_size,i.body_painting_side,i.body_painting_clip,i.body_painting_mode,i.town_effect]})
}
#[test]
fn lobby_projection_matches_original_apply_and_constructor_on_both_architectures() {
    let fixtures: Fixtures =
        serde_json::from_str(include_str!("fixtures/framework-lobby-costume.json")).unwrap();
    for p in fixtures.profiles {
        assert_eq!(p.cases.len(), 256);
        for c in p.cases {
            let rows = synthetic_rows(c.n);
            let input = serde_json::to_value(&rows).unwrap();
            assert_eq!(
                format!("{:x}", Sha256::digest(serde_json::to_vec(&input).unwrap())),
                c.input_sha256,
                "{} synthetic input {}",
                p.profile,
                c.n
            );
            let rows = CostumeSet::load(rows.into_iter().map(|r| Costume {
                is_base: r.is_base,
                kind: r.kind,
                key: r.key,
                serial: r.serial,
                colors: r.colors,
                gloss: r.gloss,
                effect: r.effect,
            }))
            .unwrap();
            let output = snapshot(lobby_costume::project(&rows).unwrap());
            assert_eq!(
                format!("{:x}", Sha256::digest(serde_json::to_vec(&output).unwrap())),
                c.expected_sha256,
                "{} output {}",
                p.profile,
                c.n
            );
        }
    }
}
#[test]
fn lobby_projection_keeps_empty_dictionaries_and_refuses_wire_bound_overflow() {
    let empty = CostumeSet::default();
    let info = lobby_costume::project(&empty).unwrap();
    assert_eq!(info.body_shape_info, Some(vec![]));
    assert_eq!(info.costume_type_info, Some(vec![]));
    assert_eq!(info.height, -1);
    let rows = CostumeSet::load((0..43).map(|key| Costume {
        is_base: true,
        kind: 0,
        key,
        serial: None,
        colors: [Some(1); 3],
        gloss: None,
        effect: None,
    }))
    .unwrap();
    assert!(lobby_costume::project(&rows).is_err());
}

fn synthetic_rows(n: i32) -> Vec<Row> {
    fn v(a: i32, b: u8) -> i32 {
        a.wrapping_mul(1103515245)
            .wrapping_add(i32::from(b).wrapping_mul(12345))
    }
    let mut rows = Vec::new();
    for key in 0..25 {
        rows.push(Row {
            is_base: true,
            kind: 2,
            key,
            serial: if n == 0 {
                None
            } else {
                Some(if n == 1 { 1 } else { v(n, key) })
            },
            colors: [None; 3],
            gloss: None,
            effect: None,
        });
    }
    for kind in 0..4 {
        for key in 0..5 {
            rows.push(Row {
                is_base: false,
                kind,
                key,
                serial: if n % 3 == 0 {
                    None
                } else {
                    Some(v(n + 1, key))
                },
                colors: [
                    if n % 2 == 0 { None } else { Some(v(n, key)) },
                    Some(v(n + 2, key)),
                    Some(n),
                ],
                gloss: Some(n as u8),
                effect: if n % 5 == 0 { None } else { Some(n) },
            });
            if kind != 2 {
                rows.push(Row {
                    is_base: true,
                    kind,
                    key,
                    serial: Some(v(n + 3, key)),
                    colors: [Some(n), None, Some(-1)],
                    gloss: None,
                    effect: Some(-7),
                });
            }
        }
    }
    rows.push(Row {
        is_base: true,
        kind: 255,
        key: 0,
        serial: Some(7),
        colors: [None; 3],
        gloss: None,
        effect: None,
    });
    rows
}
