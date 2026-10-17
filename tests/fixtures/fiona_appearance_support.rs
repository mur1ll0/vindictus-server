use vindictus_server::protocol::character_creation::{CustomizeItem, Template};
pub fn fixture() -> Value {
    let sources:Value=serde_json::from_str(include_str!("../../src/domain/fiona_gear_sources.json")).unwrap();
    let sources:serde_json::Map<String, Value>=sources.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v["sha256"].clone())).collect();
    json!({"format_version":1,"source":"heroesContents.fiona-appearance","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"implementation_sha256":"c".repeat(64),"gear_sources":sources,"features":[{"key":"Avatar","value":null},{"key":"AvatarSlotBonus","value":null},{"key":"CurrentVer","value":"3.09.00"},{"key":"LanguageTag","value":"STUDY-A"}],"equips":[{"source_index":0,"item_utf16":key("synthetic_bag"),"feature":null,"costume_type":29}],"custom":[{"item_utf16":key("synthetic_bag"),"feature":null,"cash_type":null,"is_cash":false}],"colors":[{"category_utf16":key("skin"),"feature":null,"cash_type":null,"is_cash":true,"order":7,"color":10},{"category_utf16":key("eye"),"feature":null,"cash_type":null,"is_cash":true,"order":0,"color":20},{"category_utf16":key("skin_sss"),"feature":null,"cash_type":null,"is_cash":false,"order":7,"color":30}],"parts":[{"category_utf16":key("WEAPON"),"equip_part":1,"costume_part":4,"decoration":false,"consumable":false}],"inner_item_utf16":key("synthetic_bag"),"inner_colors":[7,8,9]})
}
pub fn template() -> Template<'static> {
    Template {
        character_class: 1,
        items: Some(vec![Some(CustomizeItem {
            item_class: Some("synthetic_bag"),
            category: None,
            colors: [1, 2, 3],
            duration: 0,
            price: None,
            coupon: 0,
        })]),
        skin_color: 10,
        eye_color: 20,
        bust: 11,
        height: 12,
        shineness: 13,
        painting_x: 14,
        painting_y: 15,
        painting_rotation: 16,
        painting_size: 17,
        body_shape: Some(vec![(9, 0.5_f32.to_bits())]),
    }
}
