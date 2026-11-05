use std::{fs, time::SystemTime};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{
    cli::{UnlockArgs, UnlockModeArg},
    profile,
};

const UNLOCK_SCHEMA: u32 = 1;
const ENFORCEMENT_MARKER: &str = "metadata-only-not-yet-wired-to-gameplay-hooks";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnlockMode {
    Authentic,
    All,
    Custom,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Policy {
    pub mode: UnlockMode,
    pub allow: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UnlockMetadata {
    pub schema: u32,
    pub revision: u64,
    pub enforcement: String,
    pub generated_unix: u64,
    pub current: Policy,
    pub history: Vec<Policy>,
}

pub fn run(args: UnlockArgs) -> Result<()> {
    let profile = profile::load(&args.profile)?;
    let path = profile.root()?.join("config/unlock-policy.json");
    let mut metadata = read_or_default(&path)?;

    if args.revert {
        let previous = metadata
            .history
            .pop()
            .context("no previous unlock policy to restore")?;
        metadata.current = previous;
    } else {
        let next = policy_from_args(args.mode.context("unlock policy is required")?, args.allow)?;
        metadata.history.push(metadata.current.clone());
        metadata.current = next;
    }
    metadata.revision += 1;
    metadata.generated_unix = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_secs();
    profile::write_json(&path, &metadata)?;
    println!(
        "recorded {:?} policy revision {} for profile '{}'",
        metadata.current.mode, metadata.revision, profile.name
    );
    println!("policy metadata only: gameplay unlock hook enforcement is not implemented yet");
    println!("proprietary executable, PAK files, and authentic saves were not modified");
    Ok(())
}

fn read_or_default(path: &std::path::Path) -> Result<UnlockMetadata> {
    if path.exists() {
        let metadata: UnlockMetadata = serde_json::from_slice(&fs::read(path)?)
            .with_context(|| format!("invalid unlock metadata: {}", path.display()))?;
        validate_metadata(&metadata, path)?;
        return Ok(metadata);
    }
    Ok(UnlockMetadata {
        schema: UNLOCK_SCHEMA,
        revision: 0,
        enforcement: ENFORCEMENT_MARKER.to_owned(),
        generated_unix: 0,
        current: Policy {
            mode: UnlockMode::Authentic,
            allow: Vec::new(),
        },
        history: Vec::new(),
    })
}

fn policy_from_args(mode: UnlockModeArg, mut allow: Vec<String>) -> Result<Policy> {
    allow.sort();
    allow.dedup();
    for capability in &allow {
        anyhow::ensure!(
            !capability.is_empty()
                && capability.len() <= 48
                && capability
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "invalid custom capability '{capability}'"
        );
    }
    let mode = match mode {
        UnlockModeArg::Authentic => UnlockMode::Authentic,
        UnlockModeArg::All => UnlockMode::All,
        UnlockModeArg::Custom => UnlockMode::Custom,
    };
    anyhow::ensure!(
        matches!(mode, UnlockMode::Custom) || allow.is_empty(),
        "--allow is valid only with custom policy"
    );
    anyhow::ensure!(
        !matches!(mode, UnlockMode::Custom) || !allow.is_empty(),
        "custom policy requires at least one --allow capability"
    );
    Ok(Policy { mode, allow })
}

fn validate_metadata(metadata: &UnlockMetadata, path: &std::path::Path) -> Result<()> {
    anyhow::ensure!(
        metadata.schema == UNLOCK_SCHEMA,
        "unsupported unlock metadata schema in {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.enforcement == ENFORCEMENT_MARKER,
        "unlock metadata enforcement marker mismatch in {}",
        path.display()
    );
    validate_policy(&metadata.current)?;
    for policy in &metadata.history {
        validate_policy(policy)?;
    }
    Ok(())
}

fn validate_policy(policy: &Policy) -> Result<()> {
    for capability in &policy.allow {
        anyhow::ensure!(
            !capability.is_empty()
                && capability.len() <= 48
                && capability
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')),
            "invalid custom capability '{capability}'"
        );
    }
    anyhow::ensure!(
        !matches!(policy.mode, UnlockMode::Custom) || !policy.allow.is_empty(),
        "custom policy metadata requires at least one capability"
    );
    anyhow::ensure!(
        matches!(policy.mode, UnlockMode::Custom) || policy.allow.is_empty(),
        "non-custom policy metadata cannot contain custom capabilities"
    );
    Ok(())
}
