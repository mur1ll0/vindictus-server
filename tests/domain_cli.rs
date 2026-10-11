use argon2::{Argon2, PasswordHash, PasswordVerifier};
use clap::Parser;
use vindictus_server::{
    cli::{Cli, read_password},
    domain::{Username, hash_password},
    error::Error,
};

#[test]
fn username_has_case_insensitive_identity_without_changing_display() {
    let name = Username::parse("Research_User42").expect("valid name");
    assert_eq!(
        (name.display(), name.normalized()),
        ("Research_User42", "research_user42")
    );
}

#[test]
fn username_rejects_ambiguous_names_and_sql_metacharacters() {
    for value in [
        "",
        "ab",
        " user",
        "user ",
        "a-b",
        "a.b",
        "álvaro",
        "admin';--",
        "a\0b",
    ] {
        assert!(
            matches!(Username::parse(value), Err(Error::InvalidUsername)),
            "accepted {value:?}"
        );
    }
    assert!(Username::parse(&"a".repeat(33)).is_err());
}

#[test]
fn password_reader_accepts_exact_bounds_and_crlf_without_trimming_spaces() {
    let password = read_password(&b" 1234567890 \r\n"[..]).expect("valid password");
    assert_eq!(password.as_slice(), b" 1234567890 ");
    assert_eq!(
        read_password(&vec![b'x'; 128][..])
            .expect("max bytes")
            .len(),
        128
    );
}

#[test]
fn password_reader_rejects_short_multiline_nul_and_oversized_input() {
    for value in [
        &b"short"[..],
        &b"123456\n789012"[..],
        &b"123456\0abcdef"[..],
        &vec![b'x'; 1024][..],
    ] {
        assert!(matches!(read_password(value), Err(Error::InvalidPassword)));
    }
}

#[test]
fn password_hashes_are_salted_and_verify_only_the_correct_password() {
    let password = b"a long test password";
    let first = hash_password(password).expect("hash");
    let second = hash_password(password).expect("hash");
    assert_ne!(first, second);
    let hash = PasswordHash::new(&first).expect("PHC format");
    assert!(Argon2::default().verify_password(password, &hash).is_ok());
    assert!(
        Argon2::default()
            .verify_password(b"wrong password", &hash)
            .is_err()
    );
}

#[test]
fn cli_requires_explicit_password_stdin_and_rejects_password_argument() {
    assert!(Cli::try_parse_from(["server", "create-account", "--username", "tester"]).is_err());
    assert!(
        Cli::try_parse_from([
            "server",
            "create-account",
            "--username",
            "tester",
            "--password",
            "secret"
        ])
        .is_err()
    );
    assert!(
        Cli::try_parse_from([
            "server",
            "create-account",
            "--username",
            "tester",
            "--password-stdin"
        ])
        .is_ok()
    );
}
