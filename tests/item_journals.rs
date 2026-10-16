use vindictus_server::infrastructure::item_journals::read_snapshot;
#[test]
fn journal_identity_input_is_bounded_exact_and_nullable() {
    for values in ["\"611635750\",\"4419625\"", "null,null"] {
        let v: Vec<_> = values.split(',').collect();
        let s = format!(
            r#"{{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":{},"character_last":{}}}"#,
            v[0], v[1]
        );
        assert!(read_snapshot(s.as_bytes()).is_ok());
    }
    for s in [
        r#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":"611635749","character_last":null}"#,
        r#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":null}"#,
        r#"{"format_version":1,"source":"study_heroesLog.journal-identities","item_last":null,"character_last":"9223372036854775807"}"#,
    ] {
        assert!(read_snapshot(s.as_bytes()).is_err());
    }
    assert!(read_snapshot([b' '; 4097].as_slice()).is_err());
}
