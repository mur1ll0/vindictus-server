use serde_json::{Value,json};
pub fn key(s:&str)->String{s.encode_utf16().flat_map(u16::to_le_bytes).map(|b|format!("{b:02x}")).collect()}
pub fn fixtures()->(Value,Value,Value,Value){
 let a=key("synthetic_bag");let b=key("synthetic_cash");let equips=json!([{"item_utf16":a,"feature":null}]);
 let classes=json!([{"item_utf16":a,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null},{"item_utf16":b,"feature":null,"max_stack":9,"storage":2,"bind":null,"trade_on_bind":null}]);
 let features=json!([{"key":"CurrentVer","value":"3.09.00"},{"key":"StoryTabInInventory","value":""},{"key":"BundleExceed","value":"0"},{"key":"MaxDurabilityLimit","value":"100"},{"key":"CheckEquipItemInfoVaild","value":""},{"key":"AvatarSynthesis_V2","value":""},{"key":"ingame_cashshop_tircoin_system","value":"1.1"}]);
 let catalog=json!({"format_version":1,"source":"heroesContents.item-catalog-projection","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"region":"STUDY-A","override_region":null,"equip_rows":1,"class_rows":2,"extra_rows":0},"features":features,"equips":equips,"classes":classes,"extras":[]});
 let stats=json!({"format_version":1,"source":"heroesContents.item-stats","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"rows":[{"source_index":0,"item_utf16":a,"feature":null,"stats":vec![0;18],"max_durability":200,"weight_bits":"0000000000000000","armor_hp":0}]});
 let colors=json!({"format_version":1,"source":"heroesContents.item-colors","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"rows":[{"source_index":0,"item_utf16":a,"feature":null,"materials":[null,null,null]}],"materials":[]});
 let profile=json!({"format_version":1,"source":"heroesContents.fiona-inventory","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"constants_sha256":"c".repeat(64),"gold_utf16":key("synthetic_gold"),"tircoin_utf16":key("synthetic_coin"),"features":features,"classes":[{"source_index":0,"item_utf16":a,"feature":null,"category_utf16":key("WEAPON"),"unique":0,"expire_in":null},{"source_index":1,"item_utf16":b,"feature":null,"category_utf16":key("TEST"),"unique":0,"expire_in":null}],"requests":[{"item_utf16":a,"num":1,"feature":null},{"item_utf16":b,"num":2,"feature":null},{"item_utf16":b,"num":3,"feature":null}],"variable_keys":[]});
 (catalog,stats,colors,profile)
}
