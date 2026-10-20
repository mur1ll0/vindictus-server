pub fn equipment_profile_fixture()->serde_json::Value{
 let sources:serde_json::Value=serde_json::from_str(include_str!("../../src/domain/equipped_gear_sources.json")).unwrap();
 let implementations:serde_json::Map<String,serde_json::Value>=sources.as_object().unwrap().iter().map(|(k,v)|(k.clone(),v["sha256"].clone())).collect();
 let hex=|s:&str|s.encode_utf16().flat_map(u16::to_le_bytes).map(|b|format!("{b:02x}")).collect::<String>();
 serde_json::json!({"format_version":2,"source":"heroesContents.normal-equipment","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"implementations":implementations,
 "classes":[{"source_index":0,"item_utf16":hex("synthetic_bag"),"feature":null,"category_utf16":hex("WEAPON"),"level":7,"required_level":1,"level_upper_bound":0,"class_restriction":2,"cafe_only":0,"required_skill_utf16":null,"required_skill_rank":0,"required_enhance":0,"required_enchant":0}],
 "equips":[{"source_index":0,"item_utf16":hex("synthetic_bag"),"feature":null,"equip_utf16":hex("LONGSWORD"),"serial":29}],
 "sets":[{"item_utf16":hex("synthetic_bag"),"set_utf16":hex("synthetic_set")}],
 "effects":[{"set_utf16":hex("synthetic_set"),"count":1,"target_utf16":hex("DEF"),"amount":7}]})
}
