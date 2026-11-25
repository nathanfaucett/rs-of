#![cfg(all(unix, feature = "cli", feature = "independent-contracts"))]

use std::{fs, process::Command};

#[test]
fn setup_rejects_old_local_data_without_touching_it() {
    let root = tempfile::tempdir().expect("private data directory");
    let database = root.path().join("unified.redb");
    fs::write(&database, b"retained local data").expect("retained fixture");
    let output = Command::new(env!("CARGO_BIN_EXE_unified-server"))
        .args(["setup", "--data-dir"])
        .arg(root.path())
        .env_remove("PUBLIC_BASE_URL")
        .env("PORTLESS_URL", "https://unified.localhost:1355")
        .env("PORT", "12345")
        .output()
        .expect("run setup command");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("choose a fresh setup data root"));
    assert_eq!(
        fs::read(database).expect("retained data"),
        b"retained local data"
    );
    assert_eq!(fs::read_dir(root.path()).expect("root entries").count(), 1);
}

#[test]
fn setup_rejects_plaintext_public_origin_before_writing_state() {
    let root = tempfile::tempdir().expect("private data directory");
    let output = Command::new(env!("CARGO_BIN_EXE_unified-server"))
        .args(["setup", "--data-dir"])
        .arg(root.path())
        .env_remove("PUBLIC_BASE_URL")
        .env("PORTLESS_URL", "http://unified.localhost:1355")
        .env("PORT", "12345")
        .output()
        .expect("run setup command");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("HTTPS"));
    assert_eq!(fs::read_dir(root.path()).expect("root entries").count(), 0);
}

#[test]
fn setup_defaults_to_the_previous_local_root_and_rejects_seeded_state() {
    let cwd = tempfile::tempdir().expect("private working directory");
    let data = cwd.path().join(".data");
    fs::create_dir(&data).expect("old data root");
    fs::write(data.join("unified.redb"), b"old fixture").expect("old data");
    let output = Command::new(env!("CARGO_BIN_EXE_unified-server"))
        .current_dir(cwd.path())
        .arg("setup")
        .env_remove("PUBLIC_BASE_URL")
        .env("PORTLESS_URL", "https://unified.localhost:1355")
        .env("PORT", "12345")
        .output()
        .expect("run setup command");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("choose a fresh setup data root"));
    assert_eq!(
        fs::read(data.join("unified.redb")).expect("old data"),
        b"old fixture"
    );
    assert_eq!(fs::read_dir(data).expect("root entries").count(), 1);
}
