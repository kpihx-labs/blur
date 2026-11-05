use std::{fs, process::Command};

use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn blur(home: &TempDir) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::from_std(Command::new(env!("CARGO_BIN_EXE_blur")));
    command.env("BLUR_HOME", home.path());
    command
}

#[test]
fn generated_unlock_metadata_is_explicit_and_reversible() {
    let home = TempDir::new().unwrap();
    blur(&home)
        .args(["profile", "create", "driver"])
        .assert()
        .success();
    blur(&home)
        .args(["unlock", "--profile", "driver", "all"])
        .assert()
        .success();
    blur(&home)
        .args([
            "unlock",
            "--profile",
            "driver",
            "custom",
            "--allow",
            "cars,paint",
        ])
        .assert()
        .success();
    blur(&home)
        .args(["unlock", "--profile", "driver", "--revert"])
        .assert()
        .success();

    let path = home
        .path()
        .join("profiles/driver/config/unlock-policy.json");
    let metadata: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(metadata["revision"], 3);
    assert_eq!(metadata["current"]["mode"], "all");
    assert_eq!(
        metadata["enforcement"],
        "metadata-only-not-yet-wired-to-gameplay-hooks"
    );
    assert_eq!(metadata["history"].as_array().unwrap().len(), 1);
}

#[test]
fn unlock_rejects_invalid_enforcement_metadata_without_replacing_it() {
    let home = TempDir::new().unwrap();
    blur(&home)
        .args(["profile", "create", "driver"])
        .assert()
        .success();

    let path = home
        .path()
        .join("profiles/driver/config/unlock-policy.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": 1,
            "revision": 1,
            "enforcement": "wrong-marker",
            "generated_unix": 1,
            "current": { "mode": "authentic", "allow": [] },
            "history": []
        }))
        .unwrap(),
    )
    .unwrap();
    let before = fs::read(&path).unwrap();

    blur(&home)
        .args(["unlock", "--profile", "driver", "all"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("enforcement marker mismatch"));

    assert_eq!(fs::read(&path).unwrap(), before);
}
