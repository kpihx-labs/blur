use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    cli::{LanguageArgs, LanguageCommand},
    paths, profile,
};

pub const LANGUAGE_SCHEMA: u32 = 1;
pub const CATALOG_SCHEMA: u32 = 1;
const LANGUAGE_FILE: &str = "config/language.json";
const MAX_LANGUAGE_BYTES: u64 = 256 * 1024;
const BASE_LOCALE: &str = "en";
const MAX_LOCALE_BYTES: usize = 35;
const MAX_GAME_CATALOG_ENTRIES: usize = 20_000;
const MAX_GAME_CATALOG_DEPTH: u8 = 5;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageConfig {
    pub schema: u32,
    pub locale: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    pub locale: String,
    pub messages: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameCatalogCompleteness {
    Unknown,
    Partial,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameCatalogStatus {
    pub locale: String,
    pub completeness: GameCatalogCompleteness,
    pub evidence: Vec<PathBuf>,
}

pub fn run(args: LanguageArgs) -> Result<()> {
    let selected_profile = profile::load(&args.profile)?;
    match args.command {
        LanguageCommand::List => list(&selected_profile),
        LanguageCommand::Show => show(&selected_profile),
        LanguageCommand::Set(args) => {
            let locale = normalize_locale(&args.locale)?;
            let catalog = load_catalog(&selected_profile, &locale)?;
            validate_catalog(&catalog)?;
            let config = LanguageConfig {
                schema: LANGUAGE_SCHEMA,
                locale,
            };
            save(&selected_profile, &config)?;
            println!(
                "{}: {}",
                translate(&selected_profile, &config, "language.saved")?,
                config.locale
            );
            Ok(())
        }
        LanguageCommand::Validate(args) => {
            if let Some(file) = args.file {
                let bytes = read_bounded(&file)?;
                if args.catalog {
                    let catalog: Catalog = serde_json::from_slice(&bytes)
                        .with_context(|| format!("invalid catalog JSON: {}", file.display()))?;
                    validate_catalog(&catalog)?;
                    print_coverage(&selected_profile, &catalog)?;
                } else {
                    let config: LanguageConfig = serde_json::from_slice(&bytes)
                        .with_context(|| format!("invalid language JSON: {}", file.display()))?;
                    validate_config(&selected_profile, &config)?;
                }
            } else {
                let config = load(&selected_profile)?;
                validate_config(&selected_profile, &config)?;
                let catalog = load_catalog(&selected_profile, &config.locale)?;
                print_coverage(&selected_profile, &catalog)?;
            }
            println!(
                "{}",
                translate(
                    &selected_profile,
                    &load(&selected_profile)?,
                    "language.valid"
                )?
            );
            Ok(())
        }
        LanguageCommand::Import(args) => {
            let bytes = read_bounded(&args.file)?;
            if args.catalog {
                let catalog: Catalog = serde_json::from_slice(&bytes)
                    .with_context(|| format!("invalid catalog JSON: {}", args.file.display()))?;
                validate_catalog(&catalog)?;
                anyhow::ensure!(
                    !matches!(catalog.locale.as_str(), "en" | "fr"),
                    "built-in launcher catalogs cannot be replaced"
                );
                crate::atomic_json::write(
                    catalog_path(&selected_profile, &catalog.locale)?,
                    &catalog,
                )?;
                print_coverage(&selected_profile, &catalog)
            } else {
                let config: LanguageConfig = serde_json::from_slice(&bytes)
                    .with_context(|| format!("invalid language JSON: {}", args.file.display()))?;
                validate_config(&selected_profile, &config)?;
                save(&selected_profile, &config)
            }
        }
        LanguageCommand::Export(args) => {
            if let Some(locale) = args.catalog {
                let catalog = load_catalog(&selected_profile, &locale)?;
                crate::atomic_json::write(args.file, &catalog)
            } else {
                crate::atomic_json::write(args.file, &load(&selected_profile)?)
            }
        }
    }
}

pub fn default_config() -> LanguageConfig {
    LanguageConfig {
        schema: LANGUAGE_SCHEMA,
        locale: BASE_LOCALE.to_owned(),
    }
}

pub fn language_path(profile: &profile::Profile) -> Result<PathBuf> {
    Ok(profile.root()?.join(LANGUAGE_FILE))
}

pub fn load(profile: &profile::Profile) -> Result<LanguageConfig> {
    let path = language_path(profile)?;
    let bytes = read_bounded(&path).with_context(|| {
        format!(
            "profile language is missing; recreate it or import {}",
            path.display()
        )
    })?;
    let config: LanguageConfig = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid language configuration: {}", path.display()))?;
    validate_config(profile, &config)?;
    Ok(config)
}

pub fn save(profile: &profile::Profile, config: &LanguageConfig) -> Result<()> {
    validate_config(profile, config)?;
    crate::atomic_json::write(language_path(profile)?, config)
}

pub fn fingerprint(profile: &profile::Profile) -> Result<String> {
    let config = load(profile)?;
    let catalog = load_catalog(profile, &config.locale)?;
    let base = builtin_catalog(BASE_LOCALE)?;
    let material = serde_json::to_vec(&(config, catalog, base))?;
    Ok(format!("{:x}", Sha256::digest(material)))
}

pub fn locale(profile: &profile::Profile) -> Result<String> {
    Ok(load(profile)?.locale)
}

pub fn wine_locale(locale: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match language_subtag(locale)? {
        "en" => Ok(("en_US.UTF-8", "en-US", "00000409")),
        "fr" => Ok(("fr_FR.UTF-8", "fr-FR", "0000040c")),
        _ => anyhow::bail!("Wine locale propagation is not defined for '{locale}'"),
    }
}

pub fn runtime_locale(locale: &str) -> Result<&'static str> {
    match language_subtag(locale)? {
        "en" => Ok("en"),
        "fr" => Ok("fr"),
        _ => anyhow::bail!("native runtime locale propagation is not defined for '{locale}'"),
    }
}

pub fn detect_game_catalogs(game_dir: &Path) -> Result<Vec<GameCatalogStatus>> {
    let mut evidence: BTreeMap<&str, BTreeSet<PathBuf>> =
        [("en", BTreeSet::new()), ("fr", BTreeSet::new())].into();
    let mut pending = vec![(game_dir.to_path_buf(), 0_u8)];
    let mut inspected = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_GAME_CATALOG_DEPTH || inspected >= MAX_GAME_CATALOG_ENTRIES {
            continue;
        }
        for entry in fs::read_dir(&directory).with_context(|| {
            format!(
                "could not inspect game catalog directory {}",
                directory.display()
            )
        })? {
            if inspected >= MAX_GAME_CATALOG_ENTRIES {
                pending.clear();
                break;
            }
            let entry = entry?;
            inspected += 1;
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                pending.push((entry.path(), depth + 1));
                continue;
            }
            if !metadata.is_file() || metadata.len() == 0 {
                continue;
            }
            let relative = entry
                .path()
                .strip_prefix(game_dir)
                .unwrap_or(&entry.path())
                .to_path_buf();
            let filename = entry.file_name().to_string_lossy().to_ascii_lowercase();
            for locale in ["en", "fr"] {
                if is_recognized_game_catalog_filename(&filename, locale) {
                    evidence.entry(locale).or_default().insert(relative.clone());
                }
            }
        }
    }
    Ok(evidence
        .into_iter()
        .map(|(locale, evidence)| {
            let evidence = evidence.into_iter().collect::<Vec<_>>();
            let filenames = evidence
                .iter()
                .filter_map(|path| path.file_name())
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
                .collect::<Vec<_>>();
            let has_text = filenames
                .iter()
                .any(|name| is_recognized_text_catalog_filename(name, locale));
            let has_audio = filenames
                .iter()
                .any(|name| name == &format!("voiceoverui_{locale}.baf"));
            let completeness = if has_text && has_audio {
                GameCatalogCompleteness::Complete
            } else if evidence.is_empty() {
                GameCatalogCompleteness::Unknown
            } else {
                GameCatalogCompleteness::Partial
            };
            GameCatalogStatus {
                locale: locale.to_owned(),
                completeness,
                evidence,
            }
        })
        .collect())
}

fn list(profile: &profile::Profile) -> Result<()> {
    let config = load(profile)?;
    for locale in available_locales(profile)? {
        let catalog = load_catalog(profile, &locale)?;
        let coverage = coverage(&catalog)?;
        println!(
            "{locale}\t{coverage:.1}%{}",
            if locale == config.locale {
                "\tactive"
            } else {
                ""
            }
        );
    }
    if let Some(game_dir) = &profile.game_dir {
        for status in detect_game_catalogs(game_dir)? {
            let key = match status.completeness {
                GameCatalogCompleteness::Complete => "language.game_complete",
                GameCatalogCompleteness::Partial => "language.game_partial",
                GameCatalogCompleteness::Unknown => "language.game_unknown",
            };
            println!(
                "game:{}\t{}\t{} evidence file(s)",
                status.locale,
                translate(profile, &config, key)?,
                status.evidence.len()
            );
        }
    }
    Ok(())
}

fn show(profile: &profile::Profile) -> Result<()> {
    let config = load(profile)?;
    let catalog = load_catalog(profile, &config.locale)?;
    println!(
        "{}: {}",
        translate(profile, &config, "language.active")?,
        config.locale
    );
    print_coverage(profile, &catalog)
}

fn print_coverage(profile: &profile::Profile, catalog: &Catalog) -> Result<()> {
    let config = load(profile).unwrap_or_else(|_| default_config());
    println!(
        "{} {}: {:.1}% ({}: {})",
        translate(profile, &config, "language.catalog")?,
        catalog.locale,
        coverage(catalog)?,
        translate(profile, &config, "language.fallback")?,
        BASE_LOCALE
    );
    Ok(())
}

fn validate_config(profile: &profile::Profile, config: &LanguageConfig) -> Result<()> {
    anyhow::ensure!(
        config.schema == LANGUAGE_SCHEMA,
        "unsupported language schema {}",
        config.schema
    );
    validate_locale(&config.locale)?;
    load_catalog(profile, &config.locale)?;
    Ok(())
}

fn validate_catalog(catalog: &Catalog) -> Result<()> {
    validate_catalog_shape(catalog)?;
    let base = builtin_catalog(BASE_LOCALE)?;
    let unknown = catalog
        .messages
        .keys()
        .filter(|key| !base.messages.contains_key(*key))
        .collect::<Vec<_>>();
    anyhow::ensure!(
        catalog.locale == BASE_LOCALE || unknown.is_empty(),
        "catalog contains unknown keys: {unknown:?}"
    );
    if catalog.locale == BASE_LOCALE {
        anyhow::ensure!(
            catalog.messages.keys().eq(base.messages.keys()),
            "English fallback catalog must define exactly the canonical key set"
        );
    }
    Ok(())
}

fn validate_catalog_shape(catalog: &Catalog) -> Result<()> {
    anyhow::ensure!(
        catalog.schema == CATALOG_SCHEMA,
        "unsupported catalog schema {}",
        catalog.schema
    );
    validate_locale(&catalog.locale)?;
    anyhow::ensure!(!catalog.messages.is_empty(), "catalog has no messages");
    for (key, value) in &catalog.messages {
        anyhow::ensure!(
            !key.is_empty() && key.is_ascii(),
            "catalog key must be non-empty ASCII"
        );
        anyhow::ensure!(
            !value.trim().is_empty() && !value.chars().any(char::is_control),
            "catalog value for '{key}' is empty or unsafe"
        );
    }
    anyhow::ensure!(
        serde_json::to_vec_pretty(catalog)?.len() < MAX_LANGUAGE_BYTES as usize,
        "serialized catalog exceeds {MAX_LANGUAGE_BYTES} bytes"
    );
    Ok(())
}

fn validate_locale(locale: &str) -> Result<()> {
    anyhow::ensure!(
        normalize_locale(locale)? == locale,
        "locale must use canonical BCP47 casing"
    );
    Ok(())
}

pub fn normalize_locale(locale: &str) -> Result<String> {
    anyhow::ensure!(
        !locale.is_empty() && locale.len() <= MAX_LOCALE_BYTES && locale.is_ascii(),
        "locale must be a 1-{MAX_LOCALE_BYTES} byte ASCII BCP47 tag"
    );
    let normalized_separators = locale.replace('_', "-");
    let subtags = normalized_separators.split('-').collect::<Vec<_>>();
    anyhow::ensure!(
        !subtags.iter().any(|subtag| subtag.is_empty()),
        "locale contains an empty BCP47 subtag"
    );
    let language = subtags[0];
    anyhow::ensure!(
        matches!(language.len(), 2 | 3) && language.bytes().all(|byte| byte.is_ascii_alphabetic()),
        "locale language must be a two- or three-letter BCP47 subtag"
    );

    let mut canonical = vec![language.to_ascii_lowercase()];
    let mut index = 1;
    if subtags.get(index).is_some_and(|subtag| {
        subtag.len() == 4 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic())
    }) {
        let script = subtags[index].to_ascii_lowercase();
        canonical.push(format!(
            "{}{}",
            script[..1].to_ascii_uppercase(),
            &script[1..]
        ));
        index += 1;
    }
    if subtags.get(index).is_some_and(|subtag| {
        (subtag.len() == 2 && subtag.bytes().all(|byte| byte.is_ascii_alphabetic()))
            || (subtag.len() == 3 && subtag.bytes().all(|byte| byte.is_ascii_digit()))
    }) {
        canonical.push(subtags[index].to_ascii_uppercase());
        index += 1;
    }
    let mut variants = BTreeSet::new();
    for subtag in &subtags[index..] {
        anyhow::ensure!(
            (4..=8).contains(&subtag.len())
                && subtag.bytes().all(|byte| byte.is_ascii_alphanumeric())
                && (subtag.len() != 4 || subtag.as_bytes()[0].is_ascii_digit()),
            "unsupported or invalid BCP47 variant '{subtag}'"
        );
        let variant = subtag.to_ascii_lowercase();
        anyhow::ensure!(
            variants.insert(variant.clone()),
            "duplicate BCP47 variant '{subtag}'"
        );
        canonical.push(variant);
    }
    Ok(canonical.join("-"))
}

fn language_subtag(locale: &str) -> Result<&str> {
    validate_locale(locale)?;
    Ok(locale
        .split('-')
        .next()
        .expect("validated locale is non-empty"))
}

fn coverage(catalog: &Catalog) -> Result<f64> {
    let base = builtin_catalog(BASE_LOCALE)?;
    let translated = base
        .messages
        .keys()
        .filter(|key| catalog.messages.contains_key(*key))
        .count();
    Ok(100.0 * translated as f64 / base.messages.len() as f64)
}

fn translate(profile: &profile::Profile, config: &LanguageConfig, key: &str) -> Result<String> {
    let selected = load_catalog(profile, &config.locale)?;
    let base = builtin_catalog(BASE_LOCALE)?;
    selected
        .messages
        .get(key)
        .or_else(|| base.messages.get(key))
        .cloned()
        .with_context(|| format!("missing canonical launcher translation key '{key}'"))
}

fn available_locales(profile: &profile::Profile) -> Result<Vec<String>> {
    let mut locales = BTreeSet::from(["en".to_owned(), "fr".to_owned()]);
    let directory = profile.root()?.join("config/catalogs");
    if directory.exists() {
        paths::ensure_path_components_safe(&directory)?;
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            if entry.file_type()?.is_file()
                && let Some(locale) = entry.path().file_stem().and_then(|value| value.to_str())
                && validate_locale(locale).is_ok()
            {
                locales.insert(locale.to_owned());
            }
        }
    }
    Ok(locales.into_iter().collect())
}

fn load_catalog(profile: &profile::Profile, locale: &str) -> Result<Catalog> {
    validate_locale(locale)?;
    if matches!(locale, "en" | "fr") {
        return builtin_catalog(locale);
    }
    let path = catalog_path(profile, locale)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => {
            let bytes = read_bounded(&path)
                .with_context(|| format!("launcher catalog '{locale}' is not installed"))?;
            let catalog: Catalog = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid launcher catalog: {}", path.display()))?;
            validate_catalog(&catalog)?;
            anyhow::ensure!(catalog.locale == locale, "catalog filename/locale mismatch");
            return Ok(catalog);
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| format!("could not inspect {}", path.display()));
        }
    }
    match language_subtag(locale)? {
        "en" => builtin_catalog("en"),
        "fr" => builtin_catalog("fr"),
        _ => anyhow::bail!("launcher catalog '{locale}' is not installed"),
    }
}

fn builtin_catalog(locale: &str) -> Result<Catalog> {
    let raw = match locale {
        "en" => include_str!("../i18n/en.json"),
        "fr" => include_str!("../i18n/fr.json"),
        _ => anyhow::bail!("unknown built-in locale '{locale}'"),
    };
    let catalog: Catalog = serde_json::from_str(raw)?;
    validate_catalog_shape(&catalog)?;
    anyhow::ensure!(
        catalog.locale == locale,
        "built-in catalog locale does not match '{locale}'"
    );
    if locale != BASE_LOCALE {
        validate_catalog(&catalog)?;
    }
    let base: Catalog = serde_json::from_str(include_str!("../i18n/en.json"))?;
    anyhow::ensure!(
        catalog.messages.keys().eq(base.messages.keys()),
        "built-in {locale} catalog must exactly cover the English key set"
    );
    Ok(catalog)
}

fn catalog_path(profile: &profile::Profile, locale: &str) -> Result<PathBuf> {
    validate_locale(locale)?;
    Ok(profile
        .root()?
        .join("config/catalogs")
        .join(format!("{locale}.json")))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    paths::ensure_path_components_safe(path)?;
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;
    anyhow::ensure!(
        !metadata.file_type().is_symlink(),
        "language path must not be a symbolic link"
    );
    anyhow::ensure!(metadata.is_file(), "language path is not a regular file");
    anyhow::ensure!(
        metadata.len() <= MAX_LANGUAGE_BYTES,
        "language file exceeds {MAX_LANGUAGE_BYTES} bytes"
    );
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_LANGUAGE_BYTES,
        "language file exceeds {MAX_LANGUAGE_BYTES} bytes"
    );
    Ok(bytes)
}

fn is_recognized_game_catalog_filename(filename: &str, locale: &str) -> bool {
    is_recognized_text_catalog_filename(filename, locale)
        || filename == format!("voiceoverui_{locale}.baf")
}

fn is_recognized_text_catalog_filename(filename: &str, locale: &str) -> bool {
    ["strings", "localisation", "localization"]
        .into_iter()
        .flat_map(|stem| {
            ["json", "xml"]
                .into_iter()
                .map(move |extension| format!("{stem}_{locale}.{extension}"))
        })
        .any(|candidate| filename == candidate)
}
