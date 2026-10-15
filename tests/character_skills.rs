mod support_skills {
    include!("fixtures/skills_support.rs");
}
use serde_json::{Value, json};
use vindictus_server::infrastructure::character_skills::{SkillError, read_snapshot};
fn read(v: &Value) -> Result<(), SkillError> {
    read_snapshot(serde_json::to_vec(v).unwrap().as_slice()).map(|_| ())
}
#[test]
fn skill_snapshot_preserves_source_edge_values_and_only_primary_keys_are_unique() {
    let base = support_skills::export();
    assert!(read(&base).is_ok());
    assert!(read(&support_skills::empty()).is_ok());
    for column in ["skills", "sp_slots"] {
        let mut v = base.clone();
        let row = v[column][0].clone();
        v[column].as_array_mut().unwrap().push(row);
        assert!(read(&v).is_err());
    }
    let mut v = base.clone();
    v["skills"][0]["skill_utf16"] = json!("00d8".repeat(50));
    assert!(read(&v).is_ok());
    v["skills"][0]["skill_utf16"] = json!("00d8".repeat(51));
    assert!(read(&v).is_err());
    let mut v = base.clone();
    v["skills"][0]["locked"] = json!(256);
    assert!(read(&v).is_err());
    let mut v = base.clone();
    v["sp_source_last_value"] = Value::Null;
    assert!(read(&v).is_err());
    let mut v = base.clone();
    v["sp_source_last_value"] = json!("6");
    assert!(read(&v).is_err());
    let mut v = base;
    v["sp_source_last_value"] = json!("2147483647");
    assert!(read(&v).is_err());
}
#[test]
fn skill_snapshot_requires_nullable_fields_and_rejects_unknown_or_oversized_private_input() {
    let base = support_skills::export();
    for key in base.as_object().unwrap().keys() {
        let mut v = base.clone();
        v.as_object_mut().unwrap().remove(key);
        assert!(read(&v).is_err(), "missing {key}");
    }
    for table in ["skills", "sp_slots"] {
        for key in base[table][0].as_object().unwrap().keys() {
            let mut v = base.clone();
            v[table][0].as_object_mut().unwrap().remove(key);
            assert!(read(&v).is_err(), "missing {table}.{key}");
        }
    }
    let mut v = base;
    v["secret_extra"] = json!("never_print_private_value");
    let e = read(&v).unwrap_err();
    assert!(!e.to_string().contains("never_print_private_value"));
    assert!(read_snapshot(vec![b' '; 16 * 1024 * 1024 + 1].as_slice()).is_err());
    assert!(read_snapshot(b"{}{}".as_slice()).is_err());
}
#[test]
fn skill_import_reads_private_stdin_before_database_configuration() {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let mut child = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["import-character-skills", "--input-stdin"])
        .env_remove("DATABASE_URL")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"secret_extra":"never_print_private_value"}"#)
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("skill snapshot"));
    assert!(!stderr.contains("never_print_private_value"));
}
