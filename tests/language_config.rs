use std::{fs, process::Command};

use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

fn blur(home: &TempDir) -> assert_cmd::Command {
    let mut command = assert_cmd::Command::from_std(Command::new(env!("CARGO_BIN_EXE_blur")));
    command.env("BLUR_HOME", home.path());
    command
}

fn profile(home: &TempDir) {
    blur(home)
        .args(["profile", "create", "driver"])
        .assert()
        .success();
}

#[test]
fn bcp47_normalization_is_canonical_and_conservative() {
    assert_eq!(blur::i18n::normalize_locale("FR_fr").unwrap(), "fr-FR");
    assert_eq!(
        blur::i18n::normalize_locale("zh_hant_tw").unwrap(),
        "zh-Hant-TW"
    );
    assert!(blur::i18n::normalize_locale("../fr").is_err());
    assert!(blur::i18n::normalize_locale("en--US").is_err());
    assert!(blur::i18n::normalize_locale("en-u-ca-gregory").is_err());
    assert_eq!(blur::i18n::runtime_locale("fr-FR").unwrap(), "fr");
    assert!(blur::i18n::runtime_locale("de-DE").is_err());
}

#[test]
fn language_set_show_validate_and_export_are_atomic_and_localised() {
    let home = TempDir::new().unwrap();
    let output_dir = TempDir::new().unwrap();
    let output = output_dir.path().join("language.json");
    profile(&home);

    blur(&home)
        .args(["language", "--profile", "driver", "set", "fr"])
        .assert()
        .success()
        .stdout(predicate::str::contains("enregistrée atomiquement"));
    blur(&home)
        .args(["language", "--profile", "driver", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Langue active du lanceur: fr"))
        .stdout(predicate::str::contains("100.0%"));
    blur(&home)
        .args(["language", "--profile", "driver", "validate"])
        .assert()
        .success();
    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "export",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    let exported: Value = serde_json::from_slice(&fs::read(output).unwrap()).unwrap();
    assert_eq!(exported["locale"], "fr");
}

#[test]
fn invalid_import_preserves_the_active_language() {
    let home = TempDir::new().unwrap();
    let import = home.path().join("bad.json");
    profile(&home);
    fs::write(&import, br#"{"schema":99,"locale":"fr"}"#).unwrap();

    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "import",
            import.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unsupported language schema"));

    let active: Value = serde_json::from_slice(
        &fs::read(home.path().join("profiles/driver/config/language.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(active["locale"], "en");
}

#[test]
fn native_game_catalog_detection_never_claims_complete_from_one_signal() {
    let game = TempDir::new().unwrap();
    fs::write(game.path().join("voiceoverui_fr.baf"), b"opaque").unwrap();
    let statuses = blur::i18n::detect_game_catalogs(game.path()).unwrap();
    let french = statuses
        .iter()
        .find(|status| status.locale == "fr")
        .unwrap();
    assert_eq!(
        french.completeness,
        blur::i18n::GameCatalogCompleteness::Partial
    );
}

#[test]
fn native_game_catalog_detection_ignores_empty_exact_name_markers() {
    let game = TempDir::new().unwrap();
    fs::write(game.path().join("strings_fr.json"), b"").unwrap();
    fs::write(game.path().join("voiceoverui_fr.baf"), b"").unwrap();

    let statuses = blur::i18n::detect_game_catalogs(game.path()).unwrap();
    let french = statuses
        .iter()
        .find(|status| status.locale == "fr")
        .unwrap();
    assert_eq!(
        french.completeness,
        blur::i18n::GameCatalogCompleteness::Unknown
    );
    assert!(french.evidence.is_empty());
}

#[test]
fn bcp47_tags_are_normalized_and_regional_tags_use_the_language_catalog() {
    let home = TempDir::new().unwrap();
    profile(&home);

    blur(&home)
        .args(["language", "--profile", "driver", "set", "FR_fr"])
        .assert()
        .success();
    let active: Value = serde_json::from_slice(
        &fs::read(home.path().join("profiles/driver/config/language.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(active["locale"], "fr-FR");
    blur(&home)
        .args(["language", "--profile", "driver", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Langue active du lanceur: fr-FR"))
        .stdout(predicate::str::contains("100.0%"));
}

#[test]
fn noncanonical_imported_tags_and_unsafe_or_oversized_inputs_are_rejected() {
    let home = TempDir::new().unwrap();
    profile(&home);

    let noncanonical = home.path().join("noncanonical.json");
    fs::write(&noncanonical, br#"{"schema":1,"locale":"fr-fr"}"#).unwrap();
    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "import",
            noncanonical.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("canonical BCP47"));

    let oversized = home.path().join("oversized.json");
    fs::write(&oversized, vec![b' '; 256 * 1024 + 1]).unwrap();
    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "validate",
            oversized.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exceeds 262144 bytes"));

    #[cfg(unix)]
    {
        let link = home.path().join("linked.json");
        std::os::unix::fs::symlink(&noncanonical, &link).unwrap();
        blur(&home)
            .args([
                "language",
                "--profile",
                "driver",
                "validate",
                link.to_str().unwrap(),
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("symlink"));

        let catalogs = home.path().join("profiles/driver/config/catalogs");
        fs::create_dir_all(&catalogs).unwrap();
        std::os::unix::fs::symlink(
            home.path().join("missing-catalog.json"),
            catalogs.join("fr-FR.json"),
        )
        .unwrap();
        blur(&home)
            .args(["language", "--profile", "driver", "set", "FR_fr"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("symlink"));
    }
}

#[test]
fn native_catalog_detection_requires_exact_concrete_filenames() {
    let game = TempDir::new().unwrap();
    fs::write(game.path().join("my_french_notes.txt"), b"not a catalog").unwrap();
    fs::write(game.path().join("voiceoverui_fr.baf.backup"), b"not audio").unwrap();
    let statuses = blur::i18n::detect_game_catalogs(game.path()).unwrap();
    let french = statuses
        .iter()
        .find(|status| status.locale == "fr")
        .unwrap();
    assert_eq!(
        french.completeness,
        blur::i18n::GameCatalogCompleteness::Unknown
    );
    assert!(french.evidence.is_empty());

    fs::write(game.path().join("strings_fr.json"), b"concrete text").unwrap();
    fs::write(game.path().join("voiceoverui_fr.baf"), b"concrete audio").unwrap();
    let statuses = blur::i18n::detect_game_catalogs(game.path()).unwrap();
    let french = statuses
        .iter()
        .find(|status| status.locale == "fr")
        .unwrap();
    assert_eq!(
        french.completeness,
        blur::i18n::GameCatalogCompleteness::Complete
    );
}

#[test]
fn partial_custom_catalog_import_uses_strict_english_fallback_and_exports() {
    let home = TempDir::new().unwrap();
    let catalog = home.path().join("de.json");
    let exported = home.path().join("de-export.json");
    profile(&home);
    fs::write(
        &catalog,
        br#"{"schema":1,"locale":"de","messages":{"language.active":"Aktive Sprache"}}"#,
    )
    .unwrap();

    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "import",
            catalog.to_str().unwrap(),
            "--catalog",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("strict fallback: en"));
    blur(&home)
        .args(["language", "--profile", "driver", "set", "de"])
        .assert()
        .success();
    blur(&home)
        .args(["language", "--profile", "driver", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Aktive Sprache: de"))
        .stdout(predicate::str::contains("11.1%"));
    blur(&home)
        .args([
            "language",
            "--profile",
            "driver",
            "export",
            exported.to_str().unwrap(),
            "--catalog",
            "de",
        ])
        .assert()
        .success();
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(catalog).unwrap()).unwrap(),
        serde_json::from_slice::<Value>(&fs::read(exported).unwrap()).unwrap()
    );
}
