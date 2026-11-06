use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::{
    EXPECTED_GAME_SHA256,
    cli::{DoctorArgs, PerfArgs, PlayArgs, RuntimeChoice},
    i18n, input, paths, profile,
};

const NATIVE_EXECUTABLE: &str = "game/extracted/BLUR/Blur.exe";
const NATIVE_RUNTIME_ARTIFACT: &str = "port/harness/blurrt";
const MAX_PLAYER_NAME_BYTES: usize = 15;
const WINE_OVERLAY_MARKER: &str = ".blur-overlay-ready";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NativeRootSource {
    EnvOverride,
    VendorSubmodule,
    RefsCheckout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VendorCheckoutStatus {
    pinned_revision: String,
    checked_out_revision: String,
    tracked_dirty: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VerifiedNativeRuntime {
    root: PathBuf,
    launcher: PathBuf,
    executable: PathBuf,
    runtime_artifact: PathBuf,
    executable_sha256: String,
    runtime_sha256: String,
    source: NativeRootSource,
}

pub fn doctor(args: DoctorArgs) -> Result<()> {
    let mut failures = 0;

    doctor_vendor_checkout(&mut failures);
    match verify_native_runtime() {
        Ok(runtime) => {
            check(
                "selected native launcher",
                runtime.launcher.is_file(),
                &mut failures,
            );
            check(
                "selected native runtime artifact",
                runtime.runtime_artifact.is_file(),
                &mut failures,
            );
            check(
                "selected native Blur.exe hash",
                runtime.executable_sha256 == EXPECTED_GAME_SHA256,
                &mut failures,
            );
            println!(
                "ok   native root ({:?}): {}",
                runtime.source,
                runtime.root.display()
            );
            println!(
                "ok   native runtime artifact fingerprint (input profile not selected): {}",
                runtime.runtime_sha256
            );
        }
        Err(error) => {
            eprintln!("FAIL selected native runtime: {error:#}");
            failures += 1;
        }
    }

    if let Some(name) = args.profile {
        match profile::load(&name) {
            Ok(profile) => {
                check(
                    "isolated profile save path",
                    profile.save_dir()?.is_dir(),
                    &mut failures,
                );
                check(
                    "generated overlay metadata",
                    profile.root()?.join("overlay/manifest.json").is_file(),
                    &mut failures,
                );
                match input::load(&profile) {
                    Ok(config) => {
                        check(
                            "input schema and conflicts",
                            input::validate(&config).is_ok(),
                            &mut failures,
                        );
                        println!(
                            "ok   input behavior fingerprint: {}",
                            input::mapping_hash(&config)?
                        );
                    }
                    Err(error) => {
                        eprintln!("FAIL profile input configuration: {error:#}");
                        failures += 1;
                    }
                }
                match i18n::load(&profile) {
                    Ok(language) => println!(
                        "ok   language: {} ({})",
                        language.locale,
                        i18n::fingerprint(&profile)?
                    ),
                    Err(error) => {
                        eprintln!("FAIL profile language configuration: {error:#}");
                        failures += 1;
                    }
                }
            }
            Err(error) => {
                eprintln!("FAIL profile '{name}': {error:#}");
                failures += 1;
            }
        }
    }

    println!("policy: proprietary data remains external; no in-place patching or firewall changes");
    anyhow::ensure!(failures == 0, "doctor found {failures} failed check(s)");
    Ok(())
}

pub fn play(args: PlayArgs) -> Result<()> {
    let profile = profile::load(&args.profile)?;
    let config = input::load(&profile)?;
    input::validate_for_runtime(&config, args.runtime)?;
    let status = match args.runtime {
        RuntimeChoice::Native => native(&profile, &args, None)?,
        RuntimeChoice::Wine => wine(&profile)?,
    };
    exit_result(status)
}

pub(crate) fn native(
    profile: &profile::Profile,
    args: &PlayArgs,
    player_name: Option<&str>,
) -> Result<ExitStatus> {
    let runtime = verify_native_runtime()?;
    let input_config = input::load(profile)?;
    input::validate_for_runtime(&input_config, RuntimeChoice::Native)?;
    let locale = i18n::locale(profile)?;
    let runtime_locale = i18n::runtime_locale(&locale)?;
    let player_name = player_name
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| deterministic_player_name(&profile.name));

    let mut command = Command::new(&runtime.launcher);
    command
        .current_dir(&runtime.root)
        .env("BLUR_SAVEDIR", profile.save_dir()?)
        .env("BLUR_INPUT_CONFIG", input::input_path(profile)?)
        .env("BLUR_LANG", runtime_locale)
        .env("BLUR_EXE", &runtime.executable)
        .env("BLUR_PLAYER_NAME", &player_name)
        .arg("--port")
        .arg(args.web_port.to_string());
    command.args(["--lang", runtime_locale]);
    if args.webgl2 {
        command.arg("--webgl2");
    }
    if args.no_open {
        command.arg("--no-open");
    }
    if args.lan {
        command.arg("--lan");
    }
    command.args(["--name", &player_name]);
    command
        .status()
        .context("could not launch native lifted runtime")
}

pub fn perf(args: PerfArgs) -> Result<()> {
    let profile = profile::load(&args.profile)?;
    let config = input::load(&profile)?;
    input::validate_for_runtime(&config, RuntimeChoice::Native)?;
    let locale = i18n::locale(&profile)?;
    let runtime_locale = i18n::runtime_locale(&locale)?;
    eprintln!(
        "performance instrumentation enabled: blur-web host BLUR_PROF/BLUR_FRAMESTAT/BLUR_OBJSTAT plus browser renderer perf(), renderer identity, optional GPU timer, JS heap, and browser-observed-input-change to received-Present-batch approximation"
    );
    eprintln!(
        "scope: these are process/browser observations, not display-photon latency or whole-system GPU utilization; exercise a moving race and terminate normally to collect teardown reports"
    );
    let runtime = verify_native_runtime()?;
    let player_name = deterministic_player_name(&profile.name);
    let mut command = Command::new(&runtime.launcher);
    command
        .current_dir(&runtime.root)
        .env("BLUR_SAVEDIR", profile.save_dir()?)
        .env("BLUR_PLAYER_NAME", &player_name)
        .env("BLUR_INPUT_CONFIG", input::input_path(&profile)?)
        .env("BLUR_LANG", runtime_locale)
        .env("BLUR_EXE", &runtime.executable)
        .env("BLUR_PROF", args.sample_frames.to_string())
        .env("BLUR_FRAMESTAT", "1")
        .env("BLUR_OBJSTAT", "1")
        .env("BLUR_PERF_BROWSER", "1")
        .arg("--port")
        .arg(args.web_port.to_string())
        .arg("--hud")
        .args(["--name", &player_name]);
    command.args(["--lang", runtime_locale]);
    if args.webgl2 {
        command.arg("--webgl2");
    }
    if args.no_open {
        command.arg("--no-open");
    }
    exit_result(
        command
            .status()
            .context("could not launch instrumented native runtime")?,
    )
}

fn wine(profile: &profile::Profile) -> Result<ExitStatus> {
    anyhow::ensure!(
        cfg!(unix),
        "Wine fallback is supported only on Unix hosts; Windows/native support is not implemented in the root CLI"
    );
    let game_dir = profile
        .game_dir
        .as_ref()
        .context("Wine fallback requires a profile created with --game-dir")?;
    let executable = game_dir.join("Blur.exe");
    anyhow::ensure!(
        sha256_file(&executable)? == EXPECTED_GAME_SHA256,
        "Wine fallback refuses an unsupported Blur.exe hash"
    );
    let overlay = prepare_wine_overlay(profile, game_dir)?;
    let overlay_executable = overlay.join("Blur.exe");
    anyhow::ensure!(
        sha256_file(&overlay_executable)? == EXPECTED_GAME_SHA256,
        "Wine working overlay has an unexpected Blur.exe hash"
    );
    let locale = i18n::locale(profile)?;
    let (unix_locale, locale_name, locale_id) = i18n::wine_locale(&locale)?;
    configure_wine_language(profile, unix_locale, locale_name, locale_id)?;
    Command::new("wine")
        .current_dir(&overlay)
        .env("WINEPREFIX", profile.wine_prefix()?)
        .env("LANG", unix_locale)
        .env("LC_ALL", unix_locale)
        .arg(&overlay_executable)
        .status()
        .context("could not launch Wine fallback")
}

pub fn native_compatibility_fingerprint(profile: &profile::Profile) -> Result<String> {
    let config = input::load(profile)?;
    i18n::runtime_locale(&i18n::locale(profile)?)?;
    Ok(native_compatibility_fingerprint_from_verified(
        &verify_native_runtime()?,
        &input::mapping_hash(&config)?,
        &i18n::fingerprint(profile)?,
    ))
}

fn native_compatibility_fingerprint_from_verified(
    runtime: &VerifiedNativeRuntime,
    input_hash: &str,
    language_hash: &str,
) -> String {
    crate::lan::compatibility_hash(
        &runtime.executable_sha256,
        &runtime.runtime_sha256,
        input_hash,
        language_hash,
    )
}

fn configure_wine_language(
    profile: &profile::Profile,
    unix_locale: &str,
    locale_name: &str,
    locale_id: &str,
) -> Result<()> {
    let prefix = profile.wine_prefix()?;
    for (name, value) in [("LocaleName", locale_name), ("Locale", locale_id)] {
        let status = wine_registry_command(&prefix, unix_locale, name, value)
            .status()
            .with_context(|| format!("could not configure Wine {name}"))?;
        anyhow::ensure!(
            status.success(),
            "Wine registry update for {name} exited with {status}"
        );
    }
    Ok(())
}

fn wine_registry_command(prefix: &Path, unix_locale: &str, name: &str, value: &str) -> Command {
    let mut command = Command::new("wine");
    command
        .env("WINEPREFIX", prefix)
        .env("LANG", unix_locale)
        .env("LC_ALL", unix_locale)
        .args([
            "reg",
            "add",
            r"HKCU\Control Panel\International",
            "/v",
            name,
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ]);
    command
}

fn verify_native_runtime() -> Result<VerifiedNativeRuntime> {
    let (root, source) = resolve_native_root()?;
    let launcher = root.join("play.sh");
    anyhow::ensure!(
        launcher.is_file(),
        "native launcher is missing under {}",
        root.display()
    );
    let runtime_artifact = root.join(NATIVE_RUNTIME_ARTIFACT);
    anyhow::ensure!(
        runtime_artifact.is_file(),
        "native lift is not built under {}; run the project setup first",
        root.display()
    );
    let executable = if root.join(NATIVE_EXECUTABLE).is_file() {
        root.join(NATIVE_EXECUTABLE)
    } else {
        paths::project_root()
            .join("refs/blur-web")
            .join(NATIVE_EXECUTABLE)
    };
    anyhow::ensure!(
        executable.is_file(),
        "native Blur.exe is missing from the selected root and generated refs runtime; run the project setup first"
    );
    let executable_sha256 = sha256_file(&executable)?;
    anyhow::ensure!(
        executable_sha256 == EXPECTED_GAME_SHA256,
        "selected native Blur.exe hash is unsupported: {}",
        executable.display()
    );
    Ok(VerifiedNativeRuntime {
        root,
        launcher,
        executable,
        runtime_artifact: runtime_artifact.clone(),
        executable_sha256,
        runtime_sha256: sha256_file(&runtime_artifact)?,
        source,
    })
}

fn resolve_native_root() -> Result<(PathBuf, NativeRootSource)> {
    if let Some(path) = env::var_os("BLUR_NATIVE_ROOT") {
        let raw = pathbuf_from_env(path)?;
        let root = raw
            .canonicalize()
            .with_context(|| format!("could not resolve BLUR_NATIVE_ROOT={}", raw.display()))?;
        anyhow::ensure!(root.is_dir(), "BLUR_NATIVE_ROOT must be a directory");
        return Ok((root, NativeRootSource::EnvOverride));
    }
    let vendor = paths::project_root().join("vendor/blur-web");
    if vendor.join(NATIVE_RUNTIME_ARTIFACT).is_file() {
        Ok((vendor, NativeRootSource::VendorSubmodule))
    } else {
        Ok((
            paths::project_root().join("refs/blur-web"),
            NativeRootSource::RefsCheckout,
        ))
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn deterministic_player_name(profile_name: &str) -> String {
    let mut name = profile_name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(MAX_PLAYER_NAME_BYTES)
        .collect::<String>();
    if name.is_empty() {
        name = "player".to_owned();
    }
    name
}

fn doctor_vendor_checkout(failures: &mut usize) {
    let vendor = paths::project_root().join("vendor/blur-web");
    if !vendor.exists() {
        return;
    }
    match inspect_vendor_checkout() {
        Ok(Some(status)) => {
            check(
                "checked-out vendor/blur-web HEAD matches pinned gitlink",
                status.checked_out_revision == status.pinned_revision,
                failures,
            );
            check(
                "vendor/blur-web tracked files are clean",
                !status.tracked_dirty,
                failures,
            );
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("FAIL vendor/blur-web checkout: {error:#}");
            *failures += 1;
        }
    }
}

fn inspect_vendor_checkout() -> Result<Option<VendorCheckoutStatus>> {
    let root = paths::project_root();
    let vendor = root.join("vendor/blur-web");
    if !root.join(".git").exists() || !vendor.join(".git").exists() {
        return Ok(None);
    }
    Ok(Some(VendorCheckoutStatus {
        pinned_revision: git_revision(
            root,
            ["rev-parse", "HEAD:vendor/blur-web"],
            "could not inspect the pinned vendor/blur-web revision",
        )?,
        checked_out_revision: git_revision(
            &vendor,
            ["rev-parse", "HEAD"],
            "could not inspect the checked-out vendor/blur-web revision",
        )?,
        tracked_dirty: !git_stdout(
            &vendor,
            ["status", "--porcelain", "--untracked-files=no"],
            "could not inspect the vendor/blur-web tracked-file status",
        )?
        .is_empty(),
    }))
}

fn git_revision(current_dir: &Path, args: [&str; 2], context: &str) -> Result<String> {
    let revision = git_stdout(current_dir, args, context)?;
    anyhow::ensure!(revision.len() == 40, "invalid vendor revision '{revision}'");
    Ok(revision)
}

fn git_stdout<I, S>(current_dir: &Path, args: I, context: &str) -> Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let output = Command::new("git")
        .current_dir(current_dir)
        .args(args)
        .output()
        .with_context(|| context.to_owned())?;
    anyhow::ensure!(
        output.status.success(),
        "git command failed with {}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout)
        .context("git returned non-UTF-8 output")?
        .trim()
        .to_owned();
    Ok(stdout)
}

fn pathbuf_from_env(path: std::ffi::OsString) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn prepare_wine_overlay(profile: &profile::Profile, game_dir: &Path) -> Result<PathBuf> {
    let overlay = profile.root()?.join("overlay/wine-game");
    if overlay.join("Blur.exe").is_file() && overlay.join(WINE_OVERLAY_MARKER).is_file() {
        return Ok(overlay);
    }
    if overlay.exists() {
        fs::remove_dir_all(&overlay)
            .with_context(|| format!("could not reset {}", overlay.display()))?;
    }
    copy_directory_tree(game_dir, &overlay)?;
    fs::write(
        overlay.join(WINE_OVERLAY_MARKER),
        game_dir.display().to_string(),
    )
    .with_context(|| format!("could not finalize {}", overlay.display()))?;
    Ok(overlay)
}

fn copy_directory_tree(source: &Path, destination: &Path) -> Result<()> {
    fs::create_dir_all(destination)
        .with_context(|| format!("could not create {}", destination.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("could not read {}", source.display()))?
    {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path)
            .with_context(|| format!("could not inspect {}", source_path.display()))?;
        anyhow::ensure!(
            !metadata.file_type().is_symlink(),
            "Wine overlay refuses symbolic links under {}",
            source_path.display()
        );
        if metadata.is_dir() {
            copy_directory_tree(&source_path, &destination_path)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path).with_context(|| {
                format!(
                    "could not copy {} to {}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn check(label: &str, passed: bool, failures: &mut usize) {
    if passed {
        println!("ok   {label}");
    } else {
        eprintln!("FAIL {label}");
        *failures += 1;
    }
}

fn exit_result(status: ExitStatus) -> Result<()> {
    anyhow::ensure!(status.success(), "game runtime exited with {status}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_player_names_are_stable_and_bounded() {
        assert_eq!(deterministic_player_name("default"), "default");
        assert_eq!(
            deterministic_player_name("abcdefghijklmnop"),
            "abcdefghijklmno"
        );
    }

    #[test]
    fn native_compatibility_fingerprint_uses_verified_artifact_and_game_hashes() {
        let runtime = VerifiedNativeRuntime {
            root: PathBuf::from("/tmp/native"),
            launcher: PathBuf::from("/tmp/native/play.sh"),
            executable: PathBuf::from("/tmp/native/game/extracted/BLUR/Blur.exe"),
            runtime_artifact: PathBuf::from("/tmp/native/port/harness/blurrt"),
            executable_sha256: "a".repeat(64),
            runtime_sha256: "b".repeat(64),
            source: NativeRootSource::EnvOverride,
        };
        assert_eq!(
            native_compatibility_fingerprint_from_verified(
                &runtime,
                &"c".repeat(64),
                &"d".repeat(64),
            ),
            format!(
                "{:x}",
                Sha256::digest(format!(
                    "blur-lan-v2:{}:{}:{}:{}",
                    "a".repeat(64),
                    "b".repeat(64),
                    "c".repeat(64),
                    "d".repeat(64)
                ))
            )
        );
    }

    #[test]
    fn wine_registry_updates_use_literal_arguments_without_a_shell() {
        let command = wine_registry_command(
            Path::new("/tmp/prefix;touch-pwned"),
            "fr_FR.UTF-8",
            "LocaleName",
            "fr-FR;touch-pwned",
        );
        assert_eq!(command.get_program(), "wine");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "reg",
                "add",
                r"HKCU\Control Panel\International",
                "/v",
                "LocaleName",
                "/t",
                "REG_SZ",
                "/d",
                "fr-FR;touch-pwned",
                "/f",
            ]
        );
    }
}
