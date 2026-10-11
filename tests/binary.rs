use std::process::Command;

#[test]
fn binary_help_does_not_require_a_database_or_print_secrets() {
    let output = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["--help"])
        .env_remove("DATABASE_URL")
        .env_remove("DATABASE_URL_FILE")
        .output()
        .expect("binary");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("utf8");
    assert!(help.contains("game migration incomplete"));
    assert!(help.contains("create-account"));
}

#[test]
fn binary_invalid_username_fails_before_database_access() {
    let output = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args([
            "create-account",
            "--username",
            "bad name",
            "--password-stdin",
        ])
        .env_remove("DATABASE_URL")
        .env_remove("DATABASE_URL_FILE")
        .output()
        .expect("binary");
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .expect("utf8")
            .contains("invalid username")
    );
}

#[test]
fn probe_rejects_non_loopback_before_database_access() {
    let output = Command::new(env!("CARGO_BIN_EXE_vindictus-server"))
        .args(["probe-frontend", "--bind", "0.0.0.0:27015"])
        .env_remove("DATABASE_URL")
        .env_remove("DATABASE_URL_FILE")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("requires a loopback binding"));
    assert!(!error.contains("DATABASE_URL"));
}
