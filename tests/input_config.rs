use std::{fs, process::Command};

use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn blur(home: &TempDir) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::from_std(Command::new(env!("CARGO_BIN_EXE_blur")));
    command.env("BLUR_HOME", home.path());
    command
}

fn create(home: &TempDir) {
    blur(home)
        .args(["profile", "create", "test"])
        .assert()
        .success();
}

#[test]
fn show_set_reset_import_export_validate_round_trip() {
    let home = TempDir::new().unwrap();
    let transfer = TempDir::new().unwrap();
    create(&home);

    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "validate",
            "--runtime",
            "wine",
        ])
        .assert()
        .success();
    blur(&home)
        .args(["input", "--profile", "test", "reset", "--preset", "modern"])
        .assert()
        .success();
    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "set",
            "--path",
            "/players/0/devices/1/axes/0/deadzone",
            "--value",
            "0.2",
        ])
        .assert()
        .success();

    let exported = transfer.path().join("portable-input.json");
    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "export",
            exported.to_str().unwrap(),
        ])
        .assert()
        .success();
    let value: Value = serde_json::from_slice(&fs::read(&exported).unwrap()).unwrap();
    assert_eq!(value["schema"], 1);
    assert!(value["preset"].is_null());
    assert_eq!(
        value["players"][0]["devices"][1]["axes"][0]["deadzone"],
        0.2
    );

    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "reset",
            "--preset",
            "authentic",
        ])
        .assert()
        .success();
    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "import",
            exported.to_str().unwrap(),
        ])
        .assert()
        .success();
    blur(&home)
        .args(["input", "--profile", "test", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"deadzone\": 0.2"));
}

#[test]
fn invalid_values_and_wine_inapplicability_are_never_silent() {
    let home = TempDir::new().unwrap();
    create(&home);
    blur(&home)
        .args(["input", "--profile", "test", "reset", "--preset", "modern"])
        .assert()
        .success();
    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "validate",
            "--runtime",
            "wine",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Wine cannot safely apply"));
    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "set",
            "--path",
            "/players/0/devices/1/axes/0/deadzone",
            "--value",
            "1.0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("deadzone"));
}

#[test]
fn unknown_fields_are_rejected_without_replacing_the_active_configuration() {
    let home = TempDir::new().unwrap();
    let transfer = TempDir::new().unwrap();
    create(&home);
    let imported = transfer.path().join("unknown.json");
    let active = home.path().join("profiles/test/config/input.json");
    let original = fs::read(&active).unwrap();
    let mut value: Value = serde_json::from_slice(&original).unwrap();
    value["players"][0]["devices"][0]["selctor"] = Value::String("*".to_owned());
    fs::write(&imported, serde_json::to_vec(&value).unwrap()).unwrap();

    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "import",
            imported.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown field"));
    assert_eq!(fs::read(active).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn export_refuses_symlink_destinations() {
    let home = TempDir::new().unwrap();
    let transfer = TempDir::new().unwrap();
    create(&home);
    let outside = transfer.path().join("outside.json");
    fs::write(&outside, b"preserve me").unwrap();
    let link = transfer.path().join("export.json");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    blur(&home)
        .args([
            "input",
            "--profile",
            "test",
            "export",
            link.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink/reparse"));
    assert_eq!(fs::read(outside).unwrap(), b"preserve me");
}
