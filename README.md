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

## Same-subnet LAN

`blur lan` adds a small UDP discovery control protocol around the original
serverless LAN transport. It has no daemon, cloud, account, relay, VPN, or port
forwarding dependency. By default it selects a private broadcast-capable IPv4
interface, binds UDP 47802, advertises session/player metadata plus a
verified game/runtime compatibility fingerprint, and filters incompatible peers.
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
