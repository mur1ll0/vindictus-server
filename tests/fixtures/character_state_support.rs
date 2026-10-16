use serde_json::{json,Value};
use vindictus_server::domain::legacy_user_data::SqlDateTime;
fn date(value:i64)->String{SqlDateTime::from_units300(value).unwrap().to_sql_binary().iter().map(|b|format!("{b:02x}")).collect()}
pub fn empty()->Value{json!({"format_version":1,"source":"study_heroes.character-initialization","titles":[],"goals":[],"presets":[],"micro":[],"missions":[],"assignments":[]})}
pub fn export()->Value{
 let micro=|cid:&str,received:Option<i32>|json!({"cid":cid,"micro_play_count":i32::MIN,"micro_play_time":i64::MIN.to_string(),"success_micro_play_count":i32::MAX,"success_micro_play_time":i64::MAX.to_string(),"daily_micro_play_count":-1,"daily_free_play_count":i32::MAX,"last_micro_play_started":date(SqlDateTime::MIN),"daily_report_count":i32::MIN,"last_reported_time":date(SqlDateTime::MAX),"current_quest_utf16":null,"report_received_count":received});
 json!({"format_version":1,"source":"study_heroes.character-initialization",
 "titles":[{"cid":"1","title_id":-1,"acquired":true,"acquired_time":date(SqlDateTime::MIN),"acquired_quest_utf16":"00d8","expire_ticks":"3155378975999999999"},{"cid":"2","title_id":-1,"acquired":false,"acquired_time":null,"acquired_quest_utf16":"","expire_ticks":null}],
 "goals":[{"cid":"1","title_goal_id":i32::MIN,"progress":i32::MAX},{"cid":"2","title_goal_id":i32::MIN,"progress":-1}],
 "presets":[{"title_id":1,"class_restriction":-1},{"title_id":2,"class_restriction":0}],
 "micro":[micro("1",None),micro("2",Some(i32::MIN))],
 "missions":[{"cid":"1","completed_mission_count":-1,"last_clear_ticks":"0"},{"cid":"99","completed_mission_count":i32::MAX,"last_clear_ticks":null}],"assignments":[{"cid":"1","id":i64::MIN.to_string(),"mission_utf16":"00d8","completion_progress":null,"assignment_ticks":"0","removed_ticks":null,"progress0":null,"progress1":i32::MIN,"progress2":0,"progress3":i32::MAX,"progress4":null},{"cid":"99","id":i64::MIN.to_string(),"mission_utf16":"","completion_progress":2,"assignment_ticks":null,"removed_ticks":"3155378975999999999","progress0":null,"progress1":null,"progress2":null,"progress3":null,"progress4":null}]})
}
