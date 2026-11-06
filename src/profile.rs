use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    atomic_json,
    cli::{ProfileArgs, ProfileCommand, ProfileCreateArgs},
    input, paths,
};

const PROFILE_FILE: &str = "profile.json";
const PROFILE_SCHEMA: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Profile {
    pub schema: u32,
    pub name: String,
    pub created_unix: u64,
    pub game_dir: Option<PathBuf>,
    pub source_policy: String,
}

impl Profile {
    pub fn root(&self) -> Result<PathBuf> {
        paths::profile_root(&self.name)
    }

    pub fn save_dir(&self) -> Result<PathBuf> {
        Ok(self.root()?.join("save"))
    }

    pub fn wine_prefix(&self) -> Result<PathBuf> {
        Ok(self.root()?.join("wine"))
    }
}

pub fn run(args: ProfileArgs) -> Result<()> {
    match args.command {
        ProfileCommand::Create(args) => {
            let profile = create(args)?;
            println!(
                "created profile '{}' at {}",
                profile.name,
                profile.root()?.display()
            );
            println!(
                "source policy: external and immutable; profile creation keeps generated files in the profile"
            );
            Ok(())
        }
        ProfileCommand::List => list(),
    }
}

pub fn create(args: ProfileCreateArgs) -> Result<Profile> {
    paths::validate_profile_name(&args.name)?;
    let root = paths::profile_root(&args.name)?;
    paths::ensure_path_components_safe(&root)?;
    anyhow::ensure!(!root.exists(), "profile '{}' already exists", args.name);

    let game_dir = args
        .game_dir
        .map(|path| canonical_game_dir(&path))
        .transpose()?;
    let profile = Profile {
        schema: PROFILE_SCHEMA,
        name: args.name,
        created_unix: now_unix()?,
        game_dir,
        source_policy: "external-read-only-never-patch-in-place".to_owned(),
    };

    for directory in ["save", "overlay", "config", "wine"] {
        fs::create_dir_all(root.join(directory))
            .with_context(|| format!("could not create profile {} directory", directory))?;
    }
    write_json(root.join(PROFILE_FILE), &profile)?;
    write_json(
        root.join("config/launch.json"),
        &serde_json::json!({
            "schema": 1,
            "player_name": profile.name,
            "generated": true,
            "save_dir": "../save",
            "overlay_dir": "../overlay",
            "wine_prefix": "../wine",
            "proprietary_data_copied_during_profile_creation": false,
            "runtime_generated_copy_paths": ["../overlay/wine-game"]
        }),
    )?;
    write_json(
        root.join("overlay/manifest.json"),
        &serde_json::json!({
            "schema": 1,
            "generated": true,
            "purpose": "non-proprietary runtime overlay metadata",
            "source_mutation_allowed": false,
            "entries": []
        }),
    )?;
    input::save(
        &profile,
        &input::preset(crate::cli::InputPresetArg::Authentic),
    )?;
    Ok(profile)
}

pub fn load(name: &str) -> Result<Profile> {
    let path = paths::profile_root(name)?.join(PROFILE_FILE);
    paths::ensure_file_write_path_safe(&path)?;
    let bytes = fs::read(&path).with_context(|| {
        format!(
            "profile '{}' does not exist; run `blur profile create {name}`",
            name
        )
    })?;
    let profile: Profile = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid profile metadata: {}", path.display()))?;
    anyhow::ensure!(
        profile.schema == PROFILE_SCHEMA,
        "unsupported profile metadata schema in {}",
        path.display()
    );
    anyhow::ensure!(profile.name == name, "profile metadata/name mismatch");
    Ok(profile)
}

fn list() -> Result<()> {
    let root = paths::profiles_root()?;
    if root.exists() {
        paths::ensure_path_components_safe(&root)?;
    }
    if !root.exists() {
        println!("no profiles");
        return Ok(());
    }
    let mut names = fs::read_dir(root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.path().join(PROFILE_FILE).is_file())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    names.sort();
    if names.is_empty() {
        println!("no profiles");
    } else {
        for name in names {
            println!("{name}");
        }
    }
    Ok(())
}

pub(crate) fn write_json(path: impl AsRef<Path>, value: &impl Serialize) -> Result<()> {
    atomic_json::write(path, value)
}

fn canonical_game_dir(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("game directory does not exist: {}", path.display()))?;
    anyhow::ensure!(canonical.is_dir(), "game path must be a directory");
    anyhow::ensure!(
        canonical.join("Blur.exe").is_file(),
        "game directory has no Blur.exe"
    );
    Ok(canonical)
}

fn now_unix() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}
