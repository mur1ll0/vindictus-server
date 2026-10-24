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
    json!({"format_version":7,"source":"heroesContents.item-enhance","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),
        "equips":[{"source_index":0,"item_utf16":a,"feature":"on","enhance_type":key("T1"),"enchantable":1,"quality_type":null},{"source_index":1,"item_utf16":a,"feature":"off","enhance_type":key("T2"),"enchantable":1,"quality_type":null},{"source_index":2,"item_utf16":a,"feature":"on","enhance_type":key("T3"),"enchantable":1,"quality_type":null},{"source_index":3,"item_utf16":b,"feature":null,"enhance_type":null,"enchantable":0,"quality_type":null}],
        "enhance":[info(0,"T3",1,100,Some("on")),info(1,"T3",1,200,Some("on")),info(2,"T3",2,300,Some("off")),info(3,"T3",4,400,None),info(4,"T1",1,1,None)],
        "stats":[{"source_index":0,"enhance_type":key("T3"),"level":1,"stat":key("ATK"),"value_type":key("Add"),"value":single(50.0),"base":key("ATK")},{"source_index":1,"enhance_type":key("T3"),"level":1,"stat":key("Weight"),"value_type":key("Multiply"),"value":single(-0.1),"base":key("Weight")}],
        "server":[{"source_index":0,"enhance_type":key("1"),"level":1,"success_ratio":single(1.0),"feature":"!off"},{"source_index":1,"enhance_type":key("1"),"level":1,"success_ratio":single(0.75),"feature":"on"},{"source_index":2,"enhance_type":key("2"),"level":2,"success_ratio":single(0.5),"feature":"off"}],
        "bonus":[{"source_index":0,"item_utf16":key("gevne"),"bonus":format!("{:016x}",3.0f64.to_bits()),"min":1,"max":15}],
        "instead":[{"source_index":0,"item_utf16":key("gevne"),"use_count":1,"slot":1,"min":1,"max":15}],
        "classes":[{"source_index":0,"item_utf16":a,"feature":null,"max_anti_bind":3},{"source_index":1,"item_utf16":b,"feature":null,"max_anti_bind":0}],
        "runes":[{"source_index":0,"item_utf16":key("rune"),"relation":0,"min":4,"max":15,"feature":"on"},{"source_index":1,"item_utf16":key("rune"),"relation":1,"min":1,"max":3,"feature":"off"},{"source_index":2,"item_utf16":key("rune2"),"relation":1,"min":1,"max":3,"feature":null}],
        "rune_extra":[{"source_index":0,"relation":0,"success_ratio":format!("{:016x}",0.4f64.to_bits()),"use_item_success_ratio":false,"destroy_on_success":true,"reset_rank_on_fail":false,"force_rank_down_on_fail":0}],
        "recipes":[{"source_index":0,"recipe_utf16":key("r1"),"item_utf16":a,"num":1,"type":0,"feature":null},{"source_index":1,"recipe_utf16":key("r1"),"item_utf16":b,"num":4,"type":1,"feature":null},{"source_index":2,"recipe_utf16":key("r1"),"item_utf16":key("gold"),"num":900,"type":1,"feature":null},{"source_index":3,"recipe_utf16":key("r1"),"item_utf16":b,"num":2,"type":1,"feature":null},{"source_index":4,"recipe_utf16":key("r2"),"item_utf16":b,"num":1,"type":0,"feature":null},{"source_index":5,"recipe_utf16":key("r3"),"item_utf16":a,"num":2,"type":0,"feature":null}],
        "anti_bind":[{"source_index":0,"target":key("Base"),"from_value":null,"to_value":null,"count":2,"ap":0,"feature":null},{"source_index":1,"target":key("Enhance"),"from_value":key("9"),"to_value":key("15"),"count":0,"ap":500,"feature":"off"}],
        "enchant":[],"enchant_stats":[],"enchant_dice":[],"manufacture":[],"manufacture_recipes":[],"manufacture_materials":[],"manufacture_dependencies":[],"quality_dice":[],"quality_stats":[],"skill_enhance":[],"skill_enhance_materials":[],"skill_enhance_groups":[],"features":[{"key":"on","value":"1.1"},{"key":"CraftPriceRatio","value":"1"}]})
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
    assert_eq!(contents.max_anti_bind(&a), Some(3));
    // Runes: enabled rows, the last duplicate overwrites; extras by ReleationNum.
    assert_eq!(contents.rune("rune").unwrap().relation, 0);
    assert_eq!(contents.rune("rune2").unwrap().max_level, 3);
    assert!(contents.rune_extra(0).unwrap().destroy_on_success);
    assert_eq!(contents.rune_extra(0).unwrap().success_ratio, 0.4);
    // MakeItemRecipe: one equippable target with count 1; r3 targets 2 of a class that r1
    // already maps (so r1 stays); rows of classes the catalog lacks (gold) are skipped.
    assert_eq!(
        contents.make_item_recipe("synthetic").unwrap().materials,
        [("other".to_owned(), 4), ("other".to_owned(), 2)]
    );
    assert!(
        contents
            .make_item_recipe("other")
            .unwrap()
            .materials
            .is_empty()
    );
    assert_eq!(contents.anti_bind().len(), 1);
    assert_eq!(contents.craft_price_ratio(), Some(1.0));
}

#[test]
fn enchant_constraints_may_exceed_the_catalog_text_bound() {
    // Original ItemConstraint cells reach several KiB of class names.
    let constraint = "longsword_".repeat(120);
    let mut p = profile();
    p["enchant"] = json!([{"source_index":0,"id":1,"enchant_utf16":key("keen"),"is_prefix":true,"enchant_type":key("TEMP"),"level":1,"min_arg":0,"max_arg":0,"constraint":key(&constraint),"min_ratio":25,"max_ratio":60,"destruction":0,"mini_game":10,"duration":0,"ignore_max_armor":false,"feature":null}]);
    let contents = read(&p).unwrap().bind(&catalog()).unwrap();
    assert_eq!(
        contents.enchant("keen").unwrap().constraint.as_deref(),
        Some(constraint.as_str())
    );
    p["enchant"][0]["constraint"] = json!(key(&"x".repeat(8193)));
    assert!(read(&p).unwrap().bind(&catalog()).is_err());
}

#[test]
fn fail_rewards_and_anti_bind_ranges_follow_item_contents_extension() {
    use vindictus_server::domain::{
        character_stats::Arithmetic,
        enhance::{AntiBind, EnhanceInfo, Recipe, anti_bind_contains, fail_rewards},
    };
    let info = EnhanceInfo {
        level: 9,
        material1: Some("stone".into()),
        num1: 4,
        material2: None,
        num2: 0,
        gold: 10000,
        success_ratio: 0.4,
        on_fail: Some("Destroy".into()),
        on_fail_recipe_ratio: 0.5,
        on_fail_material_num1: 21,
        effect: 9,
    };
    let recipe = Recipe {
        materials: vec![
            ("ingot".into(), 3),
            ("gold".into(), 9000),
            ("stone".into(), 4),
            ("ingot".into(), 2),
            ("dust".into(), 1),
        ],
    };
    let price = |_: &str, n: i32| Some(n);
    // MaterialClass1 first, then (int)(n × 0.5) of the summed non-gold materials (AddOrIncrease).
    assert_eq!(
        fail_rewards(&info, Some(&recipe), price, Arithmetic::FrameworkX64),
        Some(vec![("stone".into(), 23), ("ingot".into(), 2)])
    );
    assert_eq!(
        fail_rewards(&info, None, price, Arithmetic::FrameworkX64),
        Some(vec![("stone".into(), 21)])
    );
    let range = AntiBind {
        target: Some("Enhance".into()),
        from: Some("9".into()),
        to: Some("15".into()),
        count: 0,
        ap: 500,
    };
    assert_eq!(anti_bind_contains(&range, "9"), Some(true));
    assert_eq!(anti_bind_contains(&range, "16"), Some(false));
    assert_eq!(anti_bind_contains(&range, "8"), Some(false));
    let text = AntiBind {
        target: Some("ItemClass".into()),
        from: Some("Longsword".into()),
        to: Some("_x".into()),
        count: 0,
        ap: 1,
    };
    assert_eq!(anti_bind_contains(&text, "longsword_x"), Some(true));
    assert_eq!(anti_bind_contains(&text, "longsword_y"), Some(false));
    // An integer value against a non-integer bound is the source FormatException.
    assert_eq!(anti_bind_contains(&text, "3"), None);
    let base = AntiBind {
        target: Some("Base".into()),
        from: None,
        to: None,
        count: 2,
        ap: 0,
    };
    assert_eq!(anti_bind_contains(&base, ""), Some(true));
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
    version["format_version"] = json!(5);
    assert!(read(&version).is_err());
}

#[test]
fn manufacture_contents_follow_the_data_container() {
    let mut p = profile();
    let (a, b) = (key("synthetic"), key("other"));
    p["manufacture"] = json!([{"source_index":0,"manufacture_utf16":key("cooking"),"grade":0,"min":0,"max":99},{"source_index":1,"manufacture_utf16":key("cooking"),"grade":1,"min":100,"max":199}]);
    p["manufacture_recipes"] = json!([
        {"source_index":0,"manufacture_utf16":key("cooking"),"recipe_utf16":key("stew"),"experience_required":50,"grade_required":0,"base_reward_exp":2,"recipe_item_utf16":null,"auto_give_quest":null,"always_listed":1,"feature":"on"},
        {"source_index":1,"manufacture_utf16":key("cooking"),"recipe_utf16":key("stew"),"experience_required":9,"grade_required":9,"base_reward_exp":9,"recipe_item_utf16":null,"auto_give_quest":null,"always_listed":0,"feature":null}]);
    p["manufacture_materials"] = json!([
        {"source_index":0,"recipe_utf16":key("stew"),"item_utf16":a,"num":1,"type":0},
        {"source_index":1,"recipe_utf16":key("stew"),"item_utf16":b,"num":2,"type":1},
        {"source_index":2,"recipe_utf16":key("stew"),"item_utf16":b,"num":3,"type":2},
        {"source_index":3,"recipe_utf16":key("stew"),"item_utf16":key("unknown"),"num":3,"type":1},
        {"source_index":4,"recipe_utf16":key("stew"),"item_utf16":b,"num":3,"type":7},
        {"source_index":5,"recipe_utf16":key("none"),"item_utf16":b,"num":3,"type":1}]);
    p["manufacture_dependencies"] = json!([{"source_index":0,"manufacture_utf16":key("cooking_dc"),"required_utf16":key("cooking"),"required_experience":150}]);
    p["quality_dice"] = json!([{"source_index":0,"item_type_utf16":key("WEAPON_LVL_1"),"quality":1,"dice":50},{"source_index":1,"item_type_utf16":key("WEAPON_LVL_1"),"quality":2,"dice":650}]);
    for i in 0..3 {
        p["equips"][i]["quality_type"] = json!(key("WEAPON_LVL_1"));
    }
    let contents = read(&p).unwrap().bind(&catalog()).unwrap();
    let m = contents.manufacture();
    // The first duplicated recipe stays; Target/Material/KeyMaterial rows of known classes.
    let stew = m.recipe("stew").unwrap();
    assert_eq!((stew.experience_required, stew.always_listed), (50, 1));
    assert_eq!(stew.targets, [("synthetic".to_owned(), 1)]);
    assert_eq!(stew.materials, [("other".to_owned(), 5)]);
    assert_eq!(m.recipes_by_mid("cooking").unwrap().len(), 2);
    assert_eq!(
        (m.max_exp("cooking", 1), m.min_exp("cooking", 1)),
        (199, 100)
    );
    assert_eq!(
        (m.max_exp("cooking", 2), m.max_manufacture_exp("cooking")),
        (-1, 199)
    );
    assert_eq!(contents.quality_dice("WEAPON_LVL_1"), [(1, 50), (2, 650)]);
    assert_eq!(
        contents.quality_type(&bytes("synthetic")),
        Some("WEAPON_LVL_1")
    );
    // A repeated grade refuses the load like Dictionary.Add; ToDoubleDictionary keeps
    // the first position of a repeated dice quality with the last value.
    let mut twice = p.clone();
    twice["manufacture"][1]["grade"] = json!(0);
    assert!(read(&twice).unwrap().bind(&catalog()).is_err());
    let mut twice = p.clone();
    twice["quality_dice"][1]["quality"] = json!(1);
    let contents = read(&twice).unwrap().bind(&catalog()).unwrap();
    assert_eq!(contents.quality_dice("WEAPON_LVL_1"), [(1, 650)]);
    // QualityStatInfo rows by QualityType and level (none at level 0).
    let mut q = p.clone();
    q["quality_stats"] = json!([{"source_index":0,"item_type_utf16":key("WEAPON_LVL_1"),"quality":1,"stat":key("ATK"),"value_type":key("Multiply"),"value":single(-0.01),"base":key("ATK")}]);
    let contents = read(&q).unwrap().bind(&catalog()).unwrap();
    assert_eq!(contents.quality_stat_rows(&bytes("synthetic"), 1).len(), 1);
    assert!(
        contents
            .quality_stat_rows(&bytes("synthetic"), 0)
            .is_empty()
    );
    assert!(
        contents
            .quality_stat_rows(&bytes("synthetic"), 2)
            .is_empty()
    );
}

#[test]
fn skill_enhance_contents_follow_character_contents() {
    let mut p = profile();
    let row = |i: usize, key: i32, item: &str, ratio: i32| json!({"source_index":i,"group":2,"key":key,"item_utf16":self::key(item),"max_success_ratio":ratio,"max_durability":100,"repair_value":30});
    p["skill_enhance"] = json!([row(0, 1, "Stone_Lv1", 60), row(1, 1, "stone_lv1", 55)]);
    p["skill_enhance_materials"] =
        json!([{"source_index":0,"item_utf16":key("erg"),"type":0,"min":1,"max":3}]);
    p["skill_enhance_groups"] =
        json!([{"source_index":0,"skill_utf16":key("skill_a"),"groups":key("0; 2")}]);
    let contents = read(&p).unwrap().bind(&catalog()).unwrap();
    // GetSkillEnhanceInfo keeps the last match (the item class ignores case).
    assert_eq!(contents.skill_enhance(2, 1).unwrap().max_success_ratio, 55);
    assert_eq!(
        contents
            .skill_enhance_of_item("STONE_LV1")
            .unwrap()
            .max_success_ratio,
        55
    );
    assert_eq!(contents.skill_enhance_material("erg").unwrap().max, 3);
    assert_eq!(contents.skill_enhance_groups("skill_a"), [0, 2]);
    assert!(contents.skill_enhance_groups("skill_b").is_empty());
    // skillEnhanceMaterialInfo is a ToDictionary: a repeated class refuses the load.
    let mut twice = p["skill_enhance_materials"][0].clone();
    twice["source_index"] = json!(1);
    p["skill_enhance_materials"]
        .as_array_mut()
        .unwrap()
        .push(twice);
    assert!(read(&p).unwrap().bind(&catalog()).is_err());
}
