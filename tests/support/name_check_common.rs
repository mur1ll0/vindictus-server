use serde_json::{Value, json};
use vindictus_server::infrastructure::{
    character_name_proof::NameProof,
    reward_policy::{RewardPolicyPackage, read_policy},
};
pub fn content() -> RewardPolicyPackage {
    read_policy(include_bytes!("../fixtures/synthetic-reward-policy.json").as_slice()).unwrap()
}
pub fn value(c: &RewardPolicyPackage) -> Value {
    let measured: Value =
        serde_json::from_str(include_str!("../fixtures/sql-name-candidate-equality.json")).unwrap();
    json!({"format_version":1,"candidate":"Murael","content_database_sha256":c.manifest.database_sha256,"original_config_sha256":c.manifest.config_sha256,"language_tag":"en-EU","naming_rule":"^[a-zA-Z]{2,}_?[a-zA-Z]{2,}$","max_bytes":20,"repeating":-1,"protect_period":30,"japanese":false,"forbidden":false,"forbidden_filtered_rows":768,"forbidden_cultures":859,"sql_collation":"SQL_Latin1_General_CP1_CI_AS","references":measured["cases"].as_array().unwrap().iter().map(|r|json!({"source_table":"MeasuredSynthetic","utf16_hex":r["utf16_hex"],"equal":r["equal"]})).collect::<Vec<_>>()})
}
pub fn proof(c: &RewardPolicyPackage) -> NameProof {
    NameProof::read(serde_json::to_vec(&value(c)).unwrap().as_slice(), c).unwrap()
}
