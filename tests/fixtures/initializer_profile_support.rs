use serde_json::{Value,json};
use sha2::{Digest,Sha256};
use vindictus_server::infrastructure::fiona_initializer_profile::Profile;
// All values are synthetic. The manifest contains original code paths/digests only.
pub fn synthetic() -> Value {
    let code: Value =
        serde_json::from_str(include_str!("../../src/domain/fiona_initializer_sources.json")).unwrap();
    let implementations = code
        .as_object()
        .unwrap()
        .iter()
        .filter(|(k, _)| *k != "feature_matrix")
        .map(|(k, v)| (k.clone(), v["sha256"].clone()))
        .collect::<serde_json::Map<_, _>>();
    let flags = [
        ("AutoItemLogging", true),
        ("ProcessAfterDeleteItem", false),
        ("InGameGuild", true),
        ("GuildScore", false),
        ("RandomMission", false),
        ("AddTitleFromPreset", true),
        ("DisableCharacterCreation", false),
        ("ServerTrouble", false),
        ("DisableLogging", false),
        ("DisableOperationLogging", false),
        ("SharedInventory", false),
        ("Attendance_Newbie", true),
    ];
    let mut features = flags
        .iter()
        .map(|(k, b)| json!({"key":k,"value":if *b {"synthetic-on"}else{"synthetic-off"}}))
        .collect::<Vec<_>>();
    features.extend([
        json!({"key":"CurrentVer","value":"synthetic-current"}),
        json!({"key":"AttendanceEvent_Newbie","value":"7"}),
        json!({"key":"Event_newbie_character_preset_item","value":null}),
    ]);
    json!({"format_version":1,"source":"heroesContents.first-fiona-initializer","database_sha256":"a".repeat(64),"config_sha256":"b".repeat(64),"state_sha256":"c".repeat(64),"implementations":implementations,"features":features,"enabled_keys":flags.iter().map(|(k,_)|*k).collect::<Vec<_>>(),"base_stats":[1,2,3,4,5,6],"levels":[{"level":1,"required_exp":81,"feature":null}],"presets":[{"title_id":17,"class_restriction":2},{"title_id":18,"class_restriction":0}],"title_info":[{"title_id":17,"class_restriction":99,"auto_give_level":1,"feature":null},{"title_id":18,"class_restriction":2,"auto_give_level":1,"feature":null}],"skills":[{"id_utf16":"530079006e00","rank":2,"class_restriction":2,"use_type":0,"level_constraint":1,"rank_feature":null,"constraints":[],"next_rank":{"required_ap":9,"require_unlock":false},"status_effect_types":[null,null,null],"item_callback":true,"stat_callback":false,"meditation":false,"argument_rows":2,"arguments":[{"key_utf16":"4100","value":7},{"key_utf16":"4200","value":-3}],"argument_sha256":"d".repeat(64)}],"attendance":[{"version":7,"date_from_ticks":"100","date_until_ticks":"200"}]})
}
pub fn encoded(profile: &Value) -> (Vec<u8>, Vec<u8>) {
    let input = serde_json::to_vec(profile).unwrap();
    let code: Value =
        serde_json::from_str(include_str!("../../src/domain/fiona_initializer_sources.json")).unwrap();
    let evidence = json!({"format_version":1,"source":"Framework.FeatureMatrix.IsEnable","profile_sha256":format!("{:x}",Sha256::digest(&input)),"database_sha256":profile["database_sha256"],"config_sha256":profile["config_sha256"],"feature_matrix_sha256":code["feature_matrix"]["sha256"],"profiles":["Framework48-x86-en-US","Framework48-x64-en-US"],"pairs":[{"current":"synthetic-current","value":"synthetic-on","enabled":true},{"current":"synthetic-current","value":"synthetic-off","enabled":false}]});
    (input, serde_json::to_vec(&evidence).unwrap())
}
pub fn read(v: &Value) -> Result<Profile, vindictus_server::error::Error> {
    let (a, b) = encoded(v);
    Profile::read(a.as_slice(), b.as_slice())
}
