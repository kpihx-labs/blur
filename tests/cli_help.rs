use std::process::Command;

use predicates::prelude::*;

fn blur() -> assert_cmd::Command {
    assert_cmd::Command::from_std(Command::new(env!("CARGO_BIN_EXE_blur")))
}

#[test]
fn top_level_help_exposes_the_product_commands() {
    blur()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("doctor"))
        .stdout(predicate::str::contains("play"))
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("unlock"))
        .stdout(predicate::str::contains("lan"))
        .stdout(predicate::str::contains("input"))
        .stdout(predicate::str::contains("perf"));
}

#[test]
fn nested_help_is_available_without_local_game_data() {
    for arguments in [
        &["play", "--help"][..],
        &["profile", "create", "--help"][..],
        &["lan", "discover", "--help"][..],
        &["lan", "host", "--help"][..],
        &["lan", "join", "--help"][..],
        &["input", "set", "--help"][..],
        &["perf", "--help"][..],
    ] {
        blur().args(arguments).assert().success();
    }
}
