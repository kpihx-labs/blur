# Blur

Cross-platform preservation, compatibility and customization project for a
locally owned copy of Blur (Bizarre Creations, 2010).

Original owned game data is treated as immutable input. The root project keeps
profiles, saves, generated overlays, Wine prefixes, and diagnostics local under
the platform data directory (or `BLUR_HOME`). The pinned native lift also
maintains its own extracted/copied working files under its runtime root, so the
project describes those generated copies explicitly instead of claiming that
nothing is ever copied.

Two complementary runtimes are under evaluation:

1. A Unix-host Wine 32-bit fallback that runs from a per-profile generated
   working copy instead of the external source directory.
2. `blur-web`, which lifts the supported x86 executable to native code and
   exposes rendering/input through a browser without duplicating game logic.

## CLI

The Rust 2024 `blur` crate is the local control plane around the pinned
`vendor/blur-web` submodule. Build it with `cargo build --release`, then use:

```text
blur doctor [--profile NAME]
blur profile create NAME [--game-dir /owned/Blur]
blur profile list
blur play [--profile NAME] [--runtime native|wine]
blur unlock [--profile NAME] authentic|all|custom [--allow cars,paint]
blur unlock [--profile NAME] --revert
blur lan doctor|discover|host|join
blur input [--profile NAME] show|set|reset|import|export|validate
blur language [--profile NAME] list|show|set|validate|import|export
blur perf [--profile NAME] [--sample-frames 120]
```

Profiles live under the platform-local data directory (or `BLUR_HOME`) with
separate `save/`, `overlay/`, `config/`, and `wine/` paths. Generated overlay
files are metadata only for the native root flow. The launcher records an
external game directory and never patches or rewrites the source `Blur.exe`,
signed PAKs, or authentic saves in place; explicit generated copies may exist in
native runtime roots and in the Wine working overlay.

Native lifted play is the default. `--runtime wine` is an explicit fallback and
requires a profile created with `--game-dir`. Today that fallback is enforced on
Unix hosts only. Existing local native-lift assets under ignored
`refs/blur-web` are used until the pinned vendor tree has been built; source
remains pinned in `vendor/blur-web`, while ordinary launch uses the selected
runtime root rather than requiring a writable vendor checkout. Linux/Unix host
flows are the active implementation target today; native Windows parity remains
an explicit validation item, not a completed claim.

`make check` validates reproducible repository state and never requires owned
proprietary data. `make doctor` validates this machine's external game,
toolchain, and generated native runtime. `make setup` builds the pinned
`vendor/blur-web` checkout; `BLUR_NATIVE_ROOT` is the explicit override for a
separate research checkout.

Unlock commands currently implement **reversible policy metadata only**.
Authentic, all, and custom revisions are explicit and can be reverted, but the
runtime gameplay hook that enforces those policies is not complete. The CLI
does not claim that content has been unlocked.

### Configurable input

Each profile owns a versioned `config/input.json`. `blur input show` prints the
whole effective document; `set --path JSON_POINTER --value JSON` performs a
validated atomic edit; `reset --preset authentic|modern` selects an optional
preset; and `import`, `export`, and `validate [--runtime native|wine]` provide a
portable, explicit workflow. The schema represents up to four players, multiple
keyboard/gamepad devices per player, selectors, digital bindings, axis routing,
deadzones, saturation, inversion, linear/exponential/cubic steering curves, and
configurable hotkeys. Duplicate physical sources, duplicate axes, invalid
ranges, unknown targets, unsafe paths, and unsupported schema versions fail
loudly before the active file is replaced.
Gamepad selectors are either `*`, an exact browser Gamepad API `id`, or
`index:N`; keyboard devices use the browser's single `*` seat. Editing a
built-in document clears its preset marker, and Wine accepts only the exact
authentic preset rather than trusting that marker.

The blur-web browser loads the selected profile through a dedicated no-cache
local endpoint. It applies player-1 keyboard remaps, multiple device selectors,
standard or explicit gamepad buttons, axes, deadzones and curves, and hotkeys.
The current lifted host exposes only one keyboard/XInput seat: configurations
for players 2-4 remain portable but native launch refuses them while enabled.
Wine receives the original game input directly and cannot safely consume these
browser transforms; it therefore accepts only the `authentic` transparent
preset and refuses every non-applicable mapping instead of silently ignoring it.
The complete mapping hash is included in LAN compatibility fingerprints, so
behaviorally different peers are not presented as equivalent.

### Language and catalogs

Each profile owns an atomic, versioned `config/language.json`. English is the
strict canonical fallback and the built-in English and French launcher
catalogs have exact 100% key coverage. `blur language list|show|set` manages the
effective locale; `validate`, `import`, and `export` handle either the portable
language config or, with `--catalog`, an extensible profile-local catalog.
Unknown keys, unsafe paths, oversized files, unsupported schemas, and attempts
to replace built-in catalogs fail before the active configuration changes.
Locale identifiers use canonical BCP47 language/script/region/variant casing;
underscores and case are normalized by `language set`, while imports must
already be canonical. Extensions and private-use subtags are intentionally not
accepted. Regional English/French tags use their base built-in catalog unless
an exact profile-local catalog is installed.

The native lifted runtime receives the verified base language through
`BLUR_LANG` and `--lang`; blur-web maps it to the Win32
`GetUserDefaultLCID`/`GetUserDefaultLangID` values consumed by the game and to
`#lang=` for the browser wrapper and frame-time HUD. The Wine
fallback sets `LANG`/`LC_ALL` and the isolated prefix's
`HKCU\\Control Panel\\International` `LocaleName`/`Locale` values before launch.
English (`0x0409`) and French (`0x040c`) are the currently verified runtime
mappings; unsupported runtime language subtags fail before launch rather than
being silently advertised as translated. The LAN language-catalog fingerprint
includes the active catalog and canonical English fallback because either can
change effective launcher text.

`language list` scans only filenames and file metadata in the external game
directory, never reads or rewrites proprietary data, and distinguishes
`complete`, `partial`, and `unknown`. A complete native-game claim requires
both a recognized non-empty regular localized text catalog and the matching
non-empty regular `voiceoverui_<locale>.baf`; empty markers provide no
evidence, while a lone voice bank or ambiguous/packed PAK is partial or
unknown. The known supported copy keeps localization inside signed PAKs, so
complete French game text/audio is **not claimed** until those concrete
exact-name catalog markers are actually observable.

### Performance instrumentation

`blur perf` launches the native runtime with the existing blur-web
`BLUR_PROF`, `BLUR_FRAMESTAT`, `BLUR_OBJSTAT`, renderer `perf()` rings, and HUD.
Reports cover host wall/CPU time, host peak RSS where the platform exposes it,
received-Present interval percentiles, renderer identity, WebGL GPU timer values when the
extension is available, browser JS heap when Chromium exposes it, guest/object
teardown, and time from a browser-observed input-state change to receipt of the
next WebSocket batch containing a Present. The latter includes browser,
transport, and scheduling delay and is explicitly an approximation: it is not
display-photon latency. WebGPU currently reports its
renderer and frame pacing but no GPU duration because that renderer has no
equivalent timer metric. A valid release run still requires a human-driven
moving race and normal shutdown; boot-only output is not a performance claim.

## Same-subnet LAN

`blur lan` adds a small UDP discovery control protocol around the original
serverless LAN transport. It has no daemon, cloud, account, relay, VPN, or port
forwarding dependency. By default it selects a private broadcast-capable IPv4
interface, binds UDP 47802, advertises session/player metadata plus a
verified game/runtime/input/language-catalog compatibility fingerprint plus the
selected canonical BCP47 locale, and filters incompatible peers. Protocol-v1
advertisements remain safely decodable for diagnostics but are always
incompatible with protocol v2.
Use `--bind` and `--port` to select the discovery interface and port. A
non-private address is refused unless it is explicitly selected with
`--allow-public`, which only permits that bind choice and does not disable the
same-subnet filter.

The game transport itself remains the vendor implementation on UDP 47801 and
is enabled with `BLUR_NET_MESH=1`; the original in-game LAN host/join screens
remain authoritative. `blur lan join` is only a compatibility preflight for
selecting the expected session before entering the original LAN UI. Firewall
state is diagnosed only by bind behavior and instructions: the CLI never creates
or changes firewall rules.

Current limit: two physical machines still need the release-gate validation
covering discover, host, join, race, disconnect, and reconnect. The CLI
handshake rejects known-incompatible builds, stale/future packets, and sources
outside the selected subnet policy. `--allow-public` only permits an explicitly
selected non-private interface on that subnet; it is not authentication and is
still intended only for a trusted local network.
