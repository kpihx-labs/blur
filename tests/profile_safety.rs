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
fn profile_creation_is_isolated_and_does_not_copy_or_mutate_the_game() {
    let home = TempDir::new().unwrap();
    let game = TempDir::new().unwrap();
    let executable = game.path().join("Blur.exe");
    fs::write(&executable, b"owned proprietary bytes stay here").unwrap();
    let before = fs::read(&executable).unwrap();

    blur(&home)
        .args([
            "profile",
            "create",
            "safe-profile",
            "--game-dir",
            game.path().to_str().unwrap(),
        ])
        .assert()
        .success();

    let root = home.path().join("profiles/safe-profile");
    for path in ["save", "overlay", "config", "wine"] {
        assert!(
            root.join(path).is_dir(),
            "missing isolated {path} directory"
        );
    }
    assert_eq!(fs::read(&executable).unwrap(), before);
    assert!(!root.join("Blur.exe").exists());
    assert!(!root.join("overlay/Blur.exe").exists());

    let metadata: Value =
        serde_json::from_slice(&fs::read(root.join("profile.json")).unwrap()).unwrap();
    assert_eq!(
        metadata["source_policy"],
        "external-read-only-never-patch-in-place"
    );

    let launch: Value =
        serde_json::from_slice(&fs::read(root.join("config/launch.json")).unwrap()).unwrap();
    assert_eq!(
        launch["proprietary_data_copied_during_profile_creation"],
        false
    );
    assert_eq!(
        launch["runtime_generated_copy_paths"][0],
        "../overlay/wine-game"
    );
    assert!(launch.get("proprietary_data_copied").is_none());
}

#[test]
fn unsafe_profile_names_cannot_escape_the_profile_root() {
    let home = TempDir::new().unwrap();
    blur(&home)
        .args(["profile", "create", "../escape"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("profile names must"));
    assert!(!home.path().join("escape").exists());
}

#[test]
fn profiles_are_listed_in_stable_order() {
    let home = TempDir::new().unwrap();
    blur(&home)
        .args(["profile", "create", "zeta"])
        .assert()
        .success();
    blur(&home)
        .args(["profile", "create", "alpha"])
        .assert()
        .success();
    blur(&home)
        .args(["profile", "list"])
        .assert()
        .success()
        .stdout("alpha\nzeta\n");
}

#[cfg(unix)]
#[test]
fn symlinked_profile_roots_are_rejected_before_writes_escape() {
    let home = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::create_dir_all(home.path().join("profiles")).unwrap();
    std::os::unix::fs::symlink(outside.path(), home.path().join("profiles/escape")).unwrap();

    blur(&home)
        .args(["profile", "create", "escape"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("symlink/reparse"));

    assert!(!outside.path().join("profile.json").exists());
}
