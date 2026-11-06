use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    atomic_json,
    cli::{InputArgs, InputCommand, InputPresetArg, RuntimeChoice},
    paths, profile,
};

pub const INPUT_SCHEMA: u32 = 1;
const INPUT_FILE: &str = "config/input.json";
const MAX_INPUT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum DeviceKind {
    Keyboard,
    Gamepad,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputConfig {
    pub schema: u32,
    pub preset: Option<String>,
    pub players: Vec<PlayerConfig>,
    pub hotkeys: Vec<Hotkey>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerConfig {
    pub player: u8,
    pub devices: Vec<DeviceConfig>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceConfig {
    pub id: String,
    pub kind: DeviceKind,
    pub selector: String,
    pub enabled: bool,
    pub passthrough: bool,
    pub bindings: Vec<Binding>,
    pub axes: Vec<AxisMapping>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub action: String,
    pub source: DigitalSource,
    pub target: DigitalTarget,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum DigitalSource {
    Key { code: String },
    Button { index: u8 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "type", rename_all = "snake_case")]
pub enum DigitalTarget {
    Key { vk: u16 },
    Button { button: String },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AxisMapping {
    pub action: String,
    pub source_axis: u8,
    pub target_axis: String,
    pub deadzone: f32,
    pub saturation: f32,
    pub curve: SteeringCurve,
    pub exponent: f32,
    pub invert: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub enum SteeringCurve {
    Linear,
    Exponential,
    Cubic,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Hotkey {
    pub key: String,
    pub action: String,
}

pub fn run(args: InputArgs) -> Result<()> {
    let profile = profile::load(&args.profile)?;
    match args.command {
        InputCommand::Show => println!("{}", serde_json::to_string_pretty(&load(&profile)?)?),
        InputCommand::Set(args) => {
            let mut value = serde_json::to_value(load(&profile)?)?;
            let replacement: Value = serde_json::from_str(&args.value)
                .with_context(|| "--value must be valid JSON (quote strings as JSON)")?;
            let slot = value.pointer_mut(&args.path).with_context(|| {
                format!(
                    "JSON pointer '{}' does not select an existing value",
                    args.path
                )
            })?;
            *slot = replacement;
            let mut config: InputConfig = serde_json::from_value(value)
                .context("updated value does not match the input schema")?;
            // A preset name describes an exact built-in document, not ancestry.
            config.preset = None;
            validate(&config)?;
            save(&profile, &config)?;
            println!("updated {}", input_path(&profile)?.display());
        }
        InputCommand::Reset(args) => {
            let config = preset(args.preset);
            save(&profile, &config)?;
            println!("reset input configuration to {:?}", args.preset);
        }
        InputCommand::Import(args) => {
            paths::ensure_path_components_safe(&args.file)?;
            let bytes = read_bounded(&args.file)?;
            let config: InputConfig = serde_json::from_slice(&bytes)
                .with_context(|| format!("invalid input JSON in {}", args.file.display()))?;
            validate(&config)?;
            save(&profile, &config)?;
            println!("imported {}", args.file.display());
        }
        InputCommand::Export(args) => {
            atomic_json::write(&args.file, &load(&profile)?)?;
            println!("exported {}", args.file.display());
        }
        InputCommand::Validate(args) => {
            let config = load(&profile)?;
            validate(&config)?;
            if let Some(runtime) = args.runtime {
                validate_for_runtime(&config, runtime)?;
            }
            println!(
                "valid input schema {} ({})",
                config.schema,
                mapping_hash(&config)?
            );
        }
    }
    Ok(())
}

pub fn input_path(profile: &profile::Profile) -> Result<PathBuf> {
    Ok(profile.root()?.join(INPUT_FILE))
}

pub fn load(profile: &profile::Profile) -> Result<InputConfig> {
    let path = input_path(profile)?;
    paths::ensure_file_write_path_safe(&path)?;
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let config = preset(InputPresetArg::Authentic);
            save(profile, &config)?;
            return Ok(config);
        }
        Err(error) => {
            return Err(error).with_context(|| format!("could not inspect {}", path.display()));
        }
    }
    let bytes = read_bounded(&path)
        .with_context(|| format!("input configuration is unreadable at {}", path.display()))?;
    let config: InputConfig = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid input configuration: {}", path.display()))?;
    validate(&config)?;
    Ok(config)
}

pub fn save(profile: &profile::Profile, config: &InputConfig) -> Result<()> {
    validate(config)?;
    atomic_json::write(input_path(profile)?, config)
}

pub fn mapping_hash(config: &InputConfig) -> Result<String> {
    validate(config)?;
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(config)?)))
}

pub fn validate(config: &InputConfig) -> Result<()> {
    anyhow::ensure!(
        config.schema == INPUT_SCHEMA,
        "unsupported input schema {}",
        config.schema
    );
    anyhow::ensure!(
        !config.players.is_empty(),
        "at least one player is required"
    );
    anyhow::ensure!(
        config.players.len() <= 4,
        "at most four players are supported by the schema"
    );
    anyhow::ensure!(
        config.hotkeys.len() <= 64,
        "at most 64 hotkeys are supported"
    );
    let mut players = BTreeSet::new();
    for player in &config.players {
        anyhow::ensure!(
            player.player < 4,
            "player index {} is outside 0..3",
            player.player
        );
        anyhow::ensure!(
            players.insert(player.player),
            "duplicate player {}",
            player.player
        );
        anyhow::ensure!(
            player.devices.len() <= 16,
            "at most 16 devices are supported per player"
        );
        let mut ids = BTreeSet::new();
        let mut player_keyboard_sources = BTreeMap::<String, (&str, &str)>::new();
        for device in &player.devices {
            validate_text("device id", &device.id, 64)?;
            validate_text("device selector", &device.selector, 256)?;
            if matches!(device.kind, DeviceKind::Keyboard) {
                anyhow::ensure!(
                    device.selector == "*",
                    "keyboard device '{}' must use selector '*' because browsers expose one keyboard seat",
                    device.id
                );
            }
            anyhow::ensure!(
                ids.insert(&device.id),
                "duplicate device id '{}' for player {}",
                device.id,
                player.player
            );
            anyhow::ensure!(
                device.bindings.len() <= 256,
                "device '{}' has more than 256 bindings",
                device.id
            );
            anyhow::ensure!(
                device.axes.len() <= 32,
                "device '{}' has more than 32 axis mappings",
                device.id
            );
            let mut sources = BTreeMap::<DigitalSource, &str>::new();
            for binding in &device.bindings {
                validate_text("binding action", &binding.action, 64)?;
                if let Some(previous) = sources.insert(binding.source.clone(), &binding.action) {
                    anyhow::bail!(
                        "conflict on device '{}': source {:?} is assigned to '{}' and '{}'",
                        device.id,
                        binding.source,
                        previous,
                        binding.action
                    );
                }
                match (&device.kind, &binding.source, &binding.target) {
                    (
                        DeviceKind::Keyboard,
                        DigitalSource::Key { code },
                        DigitalTarget::Key { vk },
                    ) if *vk > 0 && *vk <= 255 => {
                        validate_text("keyboard code", code, 64)?;
                        if device.enabled {
                            if let Some((previous_device, previous_action)) =
                                player_keyboard_sources
                                    .insert(code.clone(), (&device.id, &binding.action))
                            {
                                anyhow::bail!(
                                    "conflict for player {}: keyboard source '{}' is assigned by device '{}' action '{}' and device '{}' action '{}'",
                                    player.player,
                                    code,
                                    previous_device,
                                    previous_action,
                                    device.id,
                                    binding.action
                                );
                            }
                        }
                    }
                    (
                        DeviceKind::Gamepad,
                        DigitalSource::Button { index },
                        DigitalTarget::Button { button },
                    ) if *index < 32 && xinput_button(button).is_some() => {}
                    _ => anyhow::bail!(
                        "binding '{}' is not applicable to {:?} device '{}'",
                        binding.action,
                        device.kind,
                        device.id
                    ),
                }
            }
            let mut axes = BTreeSet::new();
            let mut target_axes = BTreeSet::new();
            for axis in &device.axes {
                validate_text("axis action", &axis.action, 64)?;
                anyhow::ensure!(
                    matches!(device.kind, DeviceKind::Gamepad),
                    "axes are only applicable to gamepads"
                );
                anyhow::ensure!(
                    axis.source_axis < 32,
                    "source axis {} is outside 0..31",
                    axis.source_axis
                );
                anyhow::ensure!(
                    axes.insert(axis.source_axis),
                    "conflict on device '{}': source axis {} is assigned more than once",
                    device.id,
                    axis.source_axis
                );
                anyhow::ensure!(
                    matches!(
                        axis.target_axis.as_str(),
                        "left_x"
                            | "left_y"
                            | "right_x"
                            | "right_y"
                            | "left_trigger"
                            | "right_trigger"
                    ),
                    "unknown target axis '{}'",
                    axis.target_axis
                );
                anyhow::ensure!(
                    target_axes.insert(&axis.target_axis),
                    "conflict on device '{}': target axis '{}' is assigned more than once",
                    device.id,
                    axis.target_axis
                );
                anyhow::ensure!(
                    axis.deadzone >= 0.0 && axis.deadzone < 0.95,
                    "deadzone must be in [0, 0.95)"
                );
                anyhow::ensure!(
                    axis.saturation > axis.deadzone && axis.saturation <= 1.0,
                    "saturation must be greater than deadzone and at most 1"
                );
                anyhow::ensure!(
                    (1.0..=5.0).contains(&axis.exponent),
                    "curve exponent must be in [1, 5]"
                );
            }
        }
    }
    let mut hotkeys = BTreeSet::new();
    for hotkey in &config.hotkeys {
        validate_text("hotkey key", &hotkey.key, 64)?;
        anyhow::ensure!(
            hotkeys.insert(&hotkey.key),
            "hotkey conflict on '{}'",
            hotkey.key
        );
        anyhow::ensure!(
            matches!(
                hotkey.action.as_str(),
                "fullscreen" | "hud" | "release_inputs"
            ),
            "unknown hotkey action '{}'",
            hotkey.action
        );
    }
    for player in &config.players {
        for device in player.devices.iter().filter(|device| device.enabled) {
            for binding in &device.bindings {
                if let DigitalSource::Key { code } = &binding.source {
                    anyhow::ensure!(
                        !hotkeys.contains(code),
                        "conflict: key '{}' is both a hotkey and a game binding",
                        code
                    );
                }
            }
        }
    }
    Ok(())
}

pub fn validate_for_runtime(config: &InputConfig, runtime: RuntimeChoice) -> Result<()> {
    validate(config)?;
    match runtime {
        RuntimeChoice::Native => {
            let unsupported = config.players.iter().any(|player| {
                player.player != 0 && player.devices.iter().any(|device| device.enabled)
            });
            anyhow::ensure!(
                !unsupported,
                "blur-web currently exposes one XInput/keyboard seat; enabled mappings for players 2-4 are stored portably but cannot be applied by the native runtime"
            );
            let gamepads = config
                .players
                .iter()
                .filter(|player| player.player == 0)
                .flat_map(|player| &player.devices)
                .filter(|device| device.enabled && matches!(device.kind, DeviceKind::Gamepad))
                .count();
            anyhow::ensure!(
                gamepads <= 1,
                "blur-web exposes one XInput seat and cannot apply more than one enabled gamepad device without silently choosing one"
            );
        }
        RuntimeChoice::Wine => {
            anyhow::ensure!(
                config == &preset(InputPresetArg::Authentic),
                "Wine cannot safely apply browser-level remaps, curves, hotkeys, or multi-device routing; select the authentic preset or launch the native runtime. No mapping was ignored."
            );
        }
    }
    Ok(())
}

pub fn preset(preset: InputPresetArg) -> InputConfig {
    let keyboard = |modern: bool| DeviceConfig {
        id: "keyboard-primary".to_owned(),
        kind: DeviceKind::Keyboard,
        selector: "*".to_owned(),
        enabled: true,
        passthrough: !modern,
        bindings: if modern {
            [
                ("steer_left", "KeyA", 37),
                ("steer_right", "KeyD", 39),
                ("throttle", "KeyW", 38),
                ("brake", "KeyS", 40),
                ("handbrake", "Space", 32),
            ]
            .into_iter()
            .map(|(action, code, vk)| Binding {
                action: action.to_owned(),
                source: DigitalSource::Key {
                    code: code.to_owned(),
                },
                target: DigitalTarget::Key { vk },
            })
            .collect()
        } else {
            Vec::new()
        },
        axes: Vec::new(),
    };
    let modern = matches!(preset, InputPresetArg::Modern);
    let gamepad = DeviceConfig {
        id: "gamepad-primary".to_owned(),
        kind: DeviceKind::Gamepad,
        selector: "index:0".to_owned(),
        enabled: true,
        passthrough: true,
        bindings: Vec::new(),
        axes: if modern {
            [
                (
                    "steering",
                    0,
                    "left_x",
                    0.12,
                    SteeringCurve::Exponential,
                    1.6,
                ),
                ("camera_x", 2, "right_x", 0.15, SteeringCurve::Linear, 1.0),
                ("camera_y", 3, "right_y", 0.15, SteeringCurve::Linear, 1.0),
            ]
            .into_iter()
            .map(
                |(action, source_axis, target_axis, deadzone, curve, exponent)| AxisMapping {
                    action: action.to_owned(),
                    source_axis,
                    target_axis: target_axis.to_owned(),
                    deadzone,
                    saturation: 1.0,
                    curve,
                    exponent,
                    invert: target_axis.ends_with("_y"),
                },
            )
            .collect()
        } else {
            Vec::new()
        },
    };
    InputConfig {
        schema: INPUT_SCHEMA,
        preset: Some(
            match preset {
                InputPresetArg::Authentic => "authentic",
                InputPresetArg::Modern => "modern",
            }
            .to_owned(),
        ),
        players: vec![PlayerConfig {
            player: 0,
            devices: vec![keyboard(modern), gamepad],
        }],
        hotkeys: if modern {
            vec![
                Hotkey {
                    key: "F11".to_owned(),
                    action: "fullscreen".to_owned(),
                },
                Hotkey {
                    key: "F2".to_owned(),
                    action: "hud".to_owned(),
                },
            ]
        } else {
            Vec::new()
        },
    }
}

fn xinput_button(button: &str) -> Option<u16> {
    Some(match button {
        "dpad_up" => 0x1,
        "dpad_down" => 0x2,
        "dpad_left" => 0x4,
        "dpad_right" => 0x8,
        "start" => 0x10,
        "back" => 0x20,
        "left_stick" => 0x40,
        "right_stick" => 0x80,
        "left_shoulder" => 0x100,
        "right_shoulder" => 0x200,
        "a" => 0x1000,
        "b" => 0x2000,
        "x" => 0x4000,
        "y" => 0x8000,
        _ => return None,
    })
}

fn validate_text(label: &str, value: &str, maximum: usize) -> Result<()> {
    anyhow::ensure!(
        !value.trim().is_empty() && value.len() <= maximum,
        "{label} must be 1-{maximum} bytes"
    );
    anyhow::ensure!(
        value.chars().all(|character| !character.is_control()),
        "{label} contains control characters"
    );
    Ok(())
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let metadata =
        fs::metadata(path).with_context(|| format!("could not inspect {}", path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "{} is not a regular file",
        path.display()
    );
    anyhow::ensure!(
        metadata.len() <= MAX_INPUT_BYTES,
        "{} exceeds the {} byte input configuration limit",
        path.display(),
        MAX_INPUT_BYTES
    );
    fs::read(path).with_context(|| format!("could not read {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_validate_and_have_distinct_hashes() {
        let authentic = preset(InputPresetArg::Authentic);
        let modern = preset(InputPresetArg::Modern);
        validate(&authentic).unwrap();
        validate(&modern).unwrap();
        assert_ne!(
            mapping_hash(&authentic).unwrap(),
            mapping_hash(&modern).unwrap()
        );
        validate_for_runtime(&authentic, RuntimeChoice::Wine).unwrap();
        assert!(validate_for_runtime(&modern, RuntimeChoice::Wine).is_err());
    }

    #[test]
    fn duplicate_sources_are_reported_as_conflicts() {
        let mut config = preset(InputPresetArg::Modern);
        let duplicate = config.players[0].devices[0].bindings[0].clone();
        config.players[0].devices[0].bindings.push(duplicate);
        assert!(
            validate(&config)
                .unwrap_err()
                .to_string()
                .contains("conflict")
        );
    }

    #[test]
    fn native_runtime_refuses_enabled_extra_players() {
        let mut config = preset(InputPresetArg::Authentic);
        let mut second = config.players[0].clone();
        second.player = 1;
        config.players.push(second);
        assert!(validate_for_runtime(&config, RuntimeChoice::Native).is_err());
    }

    #[test]
    fn zero_deadzone_is_valid_and_duplicate_target_axes_are_not() {
        let mut config = preset(InputPresetArg::Modern);
        config.players[0].devices[1].axes[0].deadzone = 0.0;
        validate(&config).unwrap();
        let mut duplicate = config.players[0].devices[1].axes[0].clone();
        duplicate.source_axis = 7;
        config.players[0].devices[1].axes.push(duplicate);
        assert!(
            validate(&config)
                .unwrap_err()
                .to_string()
                .contains("target axis")
        );
    }

    #[test]
    fn enabled_keyboard_devices_cannot_claim_the_same_physical_key() {
        let mut config = preset(InputPresetArg::Modern);
        let mut keyboard = config.players[0].devices[0].clone();
        keyboard.id = "keyboard-secondary".to_owned();
        config.players[0].devices.push(keyboard);
        assert!(
            validate(&config)
                .unwrap_err()
                .to_string()
                .contains("keyboard source")
        );
    }

    #[test]
    fn unknown_json_fields_are_rejected() {
        let mut value = serde_json::to_value(preset(InputPresetArg::Authentic)).unwrap();
        value["players"][0]["devices"][0]["typo"] = Value::Bool(true);
        assert!(serde_json::from_value::<InputConfig>(value).is_err());
    }

    #[test]
    fn wine_requires_the_exact_authentic_document() {
        let mut config = preset(InputPresetArg::Authentic);
        config.players[0].devices[0].enabled = false;
        assert!(validate_for_runtime(&config, RuntimeChoice::Wine).is_err());
    }
}
