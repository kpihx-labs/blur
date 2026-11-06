use std::{net::Ipv4Addr, path::PathBuf, time::Duration};

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "blur",
    version,
    about = "Local-first Blur runtime and LAN compatibility-preflight tooling"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check local runtime, profile, and network prerequisites.
    Doctor(DoctorArgs),
    /// Launch Blur with an isolated profile.
    Play(PlayArgs),
    /// Create and inspect isolated profiles.
    Profile(ProfileArgs),
    /// Write reversible unlock policy metadata (runtime enforcement is not implemented yet).
    Unlock(UnlockArgs),
    /// Diagnose and use zero-service same-subnet LAN discovery.
    Lan(LanArgs),
    /// Inspect and edit versioned per-profile input mappings.
    Input(InputArgs),
    /// Launch the native runtime with measured host/browser instrumentation enabled.
    Perf(PerfArgs),
}

#[derive(Debug, Args)]
pub struct InputArgs {
    #[arg(long, default_value = "default", global = true)]
    pub profile: String,
    #[command(subcommand)]
    pub command: InputCommand,
}

#[derive(Debug, Subcommand)]
pub enum InputCommand {
    /// Print the complete effective JSON configuration.
    Show,
    /// Replace one value using an RFC 6901 JSON pointer and a JSON value.
    Set(InputSetArgs),
    /// Replace the configuration with an optional built-in preset.
    Reset(InputResetArgs),
    /// Import and validate a complete JSON configuration.
    Import(InputFileArgs),
    /// Export the complete JSON configuration without modifying it.
    Export(InputFileArgs),
    /// Validate schema, ranges, conflicts, and runtime applicability.
    Validate(InputValidateArgs),
}

#[derive(Debug, Args)]
pub struct InputSetArgs {
    /// RFC 6901 pointer, for example /players/0/devices/0/axes/0/deadzone.
    #[arg(long)]
    pub path: String,
    /// JSON value, for example 0.18, true, or "KeyW".
    #[arg(long, allow_hyphen_values = true)]
    pub value: String,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum InputPresetArg {
    Authentic,
    #[default]
    Modern,
}

#[derive(Debug, Args)]
pub struct InputResetArgs {
    #[arg(long, value_enum, default_value_t)]
    pub preset: InputPresetArg,
}

#[derive(Debug, Args)]
pub struct InputFileArgs {
    pub file: PathBuf,
}

#[derive(Debug, Args)]
pub struct InputValidateArgs {
    /// Also require that every enabled mapping is applicable to this runtime.
    #[arg(long, value_enum)]
    pub runtime: Option<RuntimeChoice>,
}

#[derive(Debug, Args)]
pub struct PerfArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,
    /// Number of game Presents in each existing blur-web host report window.
    #[arg(long, default_value_t = 120, value_parser = clap::value_parser!(u32).range(10..=4096))]
    pub sample_frames: u32,
    #[arg(long, default_value_t = 8765)]
    pub web_port: u16,
    #[arg(long)]
    pub webgl2: bool,
    #[arg(long)]
    pub no_open: bool,
}

#[derive(Debug, Args)]
pub struct DoctorArgs {
    /// Also inspect this profile.
    #[arg(long)]
    pub profile: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum RuntimeChoice {
    #[default]
    Native,
    Wine,
}

#[derive(Debug, Args)]
pub struct PlayArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,
    /// Native lifted runtime by default; Wine is an explicit Unix-only copied-overlay fallback.
    #[arg(long, value_enum, default_value_t)]
    pub runtime: RuntimeChoice,
    #[arg(long, default_value_t = 8765)]
    pub web_port: u16,
    #[arg(long)]
    pub webgl2: bool,
    #[arg(long)]
    pub no_open: bool,
    #[arg(long, hide = true)]
    pub lan: bool,
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    /// Create isolated save, overlay, Wine, and generated-config paths.
    Create(ProfileCreateArgs),
    /// List profiles without reading proprietary game data.
    List,
}

#[derive(Debug, Args)]
pub struct ProfileCreateArgs {
    pub name: String,
    /// External, locally owned game directory. It is recorded but never patched.
    #[arg(long)]
    pub game_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum UnlockModeArg {
    Authentic,
    All,
    Custom,
}

#[derive(Debug, Args)]
pub struct UnlockArgs {
    #[arg(long, default_value = "default")]
    pub profile: String,
    /// Policy to record. This does not claim that gameplay hooks enforce it yet.
    #[arg(value_enum, required_unless_present = "revert")]
    pub mode: Option<UnlockModeArg>,
    /// Custom capability names, comma-separated or repeated.
    #[arg(long, value_delimiter = ',', requires = "mode")]
    pub allow: Vec<String>,
    /// Restore the previous policy revision.
    #[arg(long, conflicts_with_all = ["mode", "allow"])]
    pub revert: bool,
}

#[derive(Debug, Args)]
pub struct LanArgs {
    #[command(subcommand)]
    pub command: LanCommand,
}

#[derive(Debug, Subcommand)]
pub enum LanCommand {
    /// Inspect private interfaces and test the discovery bind without changing the firewall.
    Doctor(LanNetworkArgs),
    /// Find compatible sessions on the selected same-subnet interface.
    Discover(LanDiscoverArgs),
    /// Advertise a session while the original LAN transport runs.
    Host(LanHostArgs),
    /// Compatibility-preflight session selection; the in-game LAN UI remains authoritative.
    Join(LanJoinArgs),
}

#[derive(Debug, Args, Clone)]
pub struct LanNetworkArgs {
    /// Local IPv4 address to bind. By default, a private broadcast-capable interface is selected.
    #[arg(long)]
    pub bind: Option<Ipv4Addr>,
    /// Discovery control port. The original game transport remains UDP 47801.
    #[arg(long, default_value_t = 47802)]
    pub port: u16,
    /// Permit an explicitly selected non-private bind address; discovery stays limited to that subnet.
    #[arg(long, requires = "bind")]
    pub allow_public: bool,
}

#[derive(Debug, Args)]
pub struct LanDiscoverArgs {
    #[command(flatten)]
    pub network: LanNetworkArgs,
    #[arg(long, default_value = "1500ms", value_parser = parse_duration)]
    pub timeout: Duration,
    /// Show incompatible advertisements instead of filtering them out.
    #[arg(long)]
    pub show_incompatible: bool,
    /// Profile whose behavioral input mapping participates in compatibility.
    #[arg(long, default_value = "default")]
    pub profile: String,
}

#[derive(Debug, Args)]
pub struct LanHostArgs {
    #[command(flatten)]
    pub network: LanNetworkArgs,
    #[arg(long, default_value = "default")]
    pub profile: String,
    #[arg(long, default_value = "Blur LAN")]
    pub session: String,
    #[arg(long)]
    pub player: Option<String>,
    #[arg(long, default_value_t = 8765)]
    pub web_port: u16,
    #[arg(long)]
    pub no_open: bool,
}

#[derive(Debug, Args)]
pub struct LanJoinArgs {
    #[command(flatten)]
    pub discovery: LanDiscoverArgs,
    /// Exact session name. If omitted, exactly one compatible session must be visible.
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, default_value_t = 8765)]
    pub web_port: u16,
    #[arg(long)]
    pub no_open: bool,
}

fn parse_duration(value: &str) -> Result<Duration, String> {
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|error| error.to_string());
    }
    if let Some(seconds) = value.strip_suffix('s') {
        return seconds
            .parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|error| error.to_string());
    }
    Err("duration must end in ms or s".to_owned())
}
