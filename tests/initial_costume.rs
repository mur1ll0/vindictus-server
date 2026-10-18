use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vindictus_server::{
    application::initial_costume::{Context, Loaded},
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
impl From<Row> for Costume {
    fn from(r: Row) -> Self {
        Self {
            is_base: r.is_base,
            kind: r.kind,
            key: r.key,
            serial: r.serial,
            colors: r.colors,
            gloss: r.gloss,
            effect: r.effect,
        }
    }
}
fn rows<'a>(rows: impl Iterator<Item = &'a Costume>) -> Value {
    json!(
        rows.map(|r| Row {
            is_base: r.is_base,
            kind: r.kind,
            key: r.key,
            serial: r.serial,
            colors: r.colors,
            gloss: r.gloss,
            effect: r.effect
        })
        .collect::<Vec<_>>()
    )
}
fn compare(arch: &str) {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/framework-initial-costume.json")).unwrap();
    let p = fixture["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["profile"] == arch)
        .unwrap();
    assert_eq!(p["cases"].as_array().unwrap().len(), 64);
    for c in p["cases"].as_array().unwrap() {
        let input: Vec<Row> = serde_json::from_value(c["rows"].clone()).unwrap();
        let set = CostumeSet::load(input.into_iter().map(Costume::from)).unwrap();
        let loaded = Loaded::load(
            set,
            Context {
                cafe_type: c["cafe_type"].as_i64().unwrap() as i32,
                is_return: c["is_return"].as_bool().unwrap(),
            },
        )
        .unwrap();
        assert_eq!(snapshot(loaded.info()), c["expected"]);
        assert_eq!(rows(loaded.base_rows()), c["base_map"]);
        assert_eq!(rows(loaded.item_rows()), c["item_map"]);
    }
}
#[test]
fn initial_costume_framework_x86() {
    compare("Framework48-x86");
}
#[test]
fn initial_costume_framework_x64() {
    compare("Framework48-x64");
}
fn snapshot(i: &Info) -> Value {
    json!({"dictionaries":[i.costume_type_info,i.color_info,i.avatar_info,i.avatar_hide_info,i.pollution_info,i.effect_info,i.decoration_info,i.decoration_color_info,i.body_shape_info],"scalars":[i.shineness,i.height,i.bust,i.painting_pos_x,i.painting_pos_y,i.painting_rotation,i.painting_size,i32::from(i.hide_head_costume),i.cafe_type,i32::from(i.is_return),i.vip_code,i.body_painting_pos_x,i.body_painting_pos_y,i.body_painting_rotation,i.body_painting_size,i.body_painting_side,i.body_painting_clip,i.body_painting_mode,i.town_effect]})
}
