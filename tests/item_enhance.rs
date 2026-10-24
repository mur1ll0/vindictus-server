use serde_json::{Value, json};
use vindictus_server::infrastructure::{item_catalog, item_enhance::Profile};

fn key(s: &str) -> String {
    s.encode_utf16()
        .flat_map(u16::to_le_bytes)
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn bytes(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}
fn single(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}
fn catalog() -> item_catalog::Package {
    let a = key("synthetic");
    let b = key("other");
    let c = json!({"format_version":1,"source":"heroesContents.item-catalog-projection","manifest":{"database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"region":"STUDY-A","override_region":null,"equip_rows":4,"class_rows":2,"extra_rows":0},"features":[{"key":"CurrentVer","value":"3.09.00"},{"key":"StoryTabInInventory","value":""},{"key":"on","value":"1.1"},{"key":"off","value":null}],"equips":[{"item_utf16":a,"feature":"on"},{"item_utf16":a,"feature":"off"},{"item_utf16":a,"feature":"on"},{"item_utf16":b,"feature":null}],"classes":[{"item_utf16":a,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null},{"item_utf16":b,"feature":null,"max_stack":1,"storage":1,"bind":null,"trade_on_bind":null}],"extras":[]});
    item_catalog::read_package(serde_json::to_vec(&c).unwrap().as_slice()).unwrap()
}
fn profile() -> Value {
    let a = key("synthetic");
    let b = key("other");
    let info = |i: usize, kind: &str, level: i32, gold: i32, feature: Option<&str>| json!({"source_index":i,"enhance_type":key(kind),"level":level,"material1":key("stone"),"num1":2,"material2":null,"num2":0,"gold":gold,"success_ratio":single(1.0),"on_fail":key("RankDown"),"on_fail_recipe_ratio":single(0.5),"on_fail_material_num1":3,"effect":1,"feature":feature});
    json!({"format_version":1,"source":"heroesContents.item-enhance","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),
        "equips":[{"source_index":0,"item_utf16":a,"feature":"on","enhance_type":key("T1")},{"source_index":1,"item_utf16":a,"feature":"off","enhance_type":key("T2")},{"source_index":2,"item_utf16":a,"feature":"on","enhance_type":key("T3")},{"source_index":3,"item_utf16":b,"feature":null,"enhance_type":null}],
        "enhance":[info(0,"T3",1,100,Some("on")),info(1,"T3",1,200,Some("on")),info(2,"T3",2,300,Some("off")),info(3,"T3",4,400,None),info(4,"T1",1,1,None)],
        "stats":[{"source_index":0,"enhance_type":key("T3"),"level":1,"stat":key("ATK"),"value_type":key("Add"),"value":single(50.0),"base":key("ATK")},{"source_index":1,"enhance_type":key("T3"),"level":1,"stat":key("Weight"),"value_type":key("Multiply"),"value":single(-0.1),"base":key("Weight")}],
        "server":[{"source_index":0,"enhance_type":key("1"),"level":1,"success_ratio":single(1.0),"feature":"!off"},{"source_index":1,"enhance_type":key("1"),"level":1,"success_ratio":single(0.75),"feature":"on"},{"source_index":2,"enhance_type":key("2"),"level":2,"success_ratio":single(0.5),"feature":"off"}],
        "bonus":[{"source_index":0,"item_utf16":key("gevne"),"bonus":format!("{:016x}",3.0f64.to_bits()),"min":1,"max":15}],
        "instead":[{"source_index":0,"item_utf16":key("gevne"),"use_count":1,"slot":1,"min":1,"max":15}],
        "features":[{"key":"on","value":"1.1"}]})
}
fn read(
    v: &Value,
) -> Result<Profile, vindictus_server::infrastructure::minimum_character::MinimumError> {
    Profile::read(serde_json::to_vec(v).unwrap().as_slice())
}

#[test]
fn enhance_contents_follow_the_effective_equipment_and_item_contents_dictionaries() {
    let catalog = catalog();
    let contents = read(&profile()).unwrap().bind(&catalog).unwrap();
    let a = bytes("synthetic");
    // The last enabled EquipItemInfo row wins, like the stats projection.
    assert_eq!(contents.enhance_type(&a), Some("T3"));
    assert_eq!(contents.enhance_type(&bytes("other")), None);
    assert_eq!(contents.enhance_type(&bytes("unknown")), None);
    // Enabled rows only; ToDoubleDictionary keeps the last duplicate.
    assert_eq!(contents.info(&a, 1).unwrap().gold, 200);
    assert!(contents.info(&a, 2).is_none());
    assert_eq!(
        contents.info(&a, 4).unwrap().on_fail.as_deref(),
        Some("RankDown")
    );
    assert_eq!(contents.info(&a, 4).unwrap().on_fail_recipe_ratio, 0.5);
    assert_eq!(contents.max_level(&a), 4);
    assert_eq!(contents.max_level(&bytes("other")), -1);
    // Stat rows keep provider order; an info without rows is Some(None).
    let rows = contents.stat_rows(&a, 1).unwrap().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].value, -0.1);
    assert_eq!(contents.stat_rows(&a, 4), Some(None));
    assert_eq!(contents.stat_rows(&a, 2), None);
    // Server ratio: last enabled row; disabled rows absent.
    assert_eq!(contents.server_ratio("1", 1), Some(0.75));
    assert_eq!(contents.server_ratio("2", 2), None);
    assert_eq!(contents.server_ratio("", 1), None);
    assert_eq!(contents.bonus("gevne", 15), Some(3.0));
    assert_eq!(contents.bonus("gevne", 16), None);
    assert_eq!(contents.instead("gevne", 1), Some(("gevne", 1)));
}

#[test]
fn enhance_profiles_refuse_other_snapshots_and_unsound_rows() {
    let catalog = catalog();
    let mut other_hash = profile();
    other_hash["database_sha256"] = json!("c".repeat(64));
    assert!(read(&other_hash).unwrap().bind(&catalog).is_err());
    let mut order = profile();
    order["equips"][0]["feature"] = json!("off");
    assert!(read(&order).unwrap().bind(&catalog).is_err());
    let mut conflict = profile();
    conflict["features"] = json!([{"key":"on","value":"2"}]);
    assert!(read(&conflict).unwrap().bind(&catalog).is_err());
    let mut null_key = profile();
    null_key["stats"][0]["enhance_type"] = Value::Null;
    assert!(read(&null_key).unwrap().bind(&catalog).is_err());
    let mut bits = profile();
    bits["stats"][0]["value"] = json!("3f80");
    assert!(read(&bits).is_err());
    let mut unordered = profile();
    unordered["enhance"][1]["source_index"] = json!(0);
    assert!(read(&unordered).is_err());
    let mut version = profile();
    version["format_version"] = json!(2);
    assert!(read(&version).is_err());
}
