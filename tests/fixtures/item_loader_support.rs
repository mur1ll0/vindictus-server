use serde_json::{Value,json};
pub fn snapshot()->Value{
 let source:Value=serde_json::from_str(include_str!("../../src/domain/item_loader_sources.json")).unwrap();
 json!({"format_version":1,"source":"study_heroes.item-loader-state","schema_sha256":source["schema_sha256"],"source_last":["11","12","13","14","15"],
 "quick_slots":[{"row_id":"11","owner_id":"1","item_class_utf16":"410000d82000","slot":-3}],
 "expanded_slots":[{"row_id":12,"cid":"1","slot":-9}],
 "manufacture":[{"row_id":13,"cid":"1","manufacture_utf16":"410061002000","grade":-1,"experience_point":2147483647}],
 "recipes":[{"row_id":14,"cid":"1","recipe_utf16":"410000d8","acquired_ticks":"3155378975999999999"}],
 "new_recipes":[{"row_id":"15","cid":"1","recipe_utf16":"200041002000","new_until_ticks":"0"}]})
}
pub fn empty()->Value{let mut v=snapshot();for n in ["quick_slots","expanded_slots","manufacture","recipes","new_recipes"]{v[n]=json!([])}v["source_last"]=json!([null,null,null,null,null]);v}
