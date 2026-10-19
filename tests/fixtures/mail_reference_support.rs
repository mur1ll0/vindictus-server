use serde_json::{json,Value};
pub fn oracle()->Value{serde_json::from_str(include_str!("sqlserver-empty-mail.json")).unwrap()}
pub fn data()->Value{
 let rows=oracle()["rows"].as_array().unwrap().iter().map(|r|json!({
 "mail_id":r["mail_id"].to_string(),"from_cid":r["from_cid"].to_string(),"from_name_hex":"00D8","to_cid":r["to_cid"].to_string(),"to_name_hex":"2000","mail_type":255,"posted_binary":"FFFFFFFF00000001","title_hex":"41000000","content_hex":"00DC","arrive_binary":"0000000000000002","expire_binary":null,"is_read":null,"charged_gold":-2147483648_i32,"is_deleted":r["is_deleted"]})).collect::<Vec<_>>();
 json!({"format_version":1,"source":"study_heroes.mail","procedure_sha256":oracle()["procedure_sha256"],"rows":rows})
}
