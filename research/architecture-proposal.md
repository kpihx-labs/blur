# Architecture proposal

## Goal

Keep the authentic Blur engine, physics, content and feel while removing OS,
DRM-host and progression constraints through adapters and runtime hooks.

## Runtime matrix

### Ubuntu primary

Use the `blur-web` native lift:

`packed x86 executable -> verified runtime dump -> lifted C -> native host`

The host owns the original simulation. A browser tab renders its D3D9 command
stream through WebGPU or WebGL2. This is not a remake and does not duplicate game
logic.

### Ubuntu fallback

Keep an isolated 32-bit Wine prefix with DXVK, VC++ 2008 and PhysX. It validates
behavior against the original executable and covers regressions in the lifted
runtime.

### Windows

Run the verified original executable directly with the maintained d3d9 hook.
Use the same Lua plugin configuration and portable save manager as Ubuntu.
The experimental blur-web Windows cross-build is not the default until tested
on real Windows.

## One customization layer

Extend the existing d3d9/Lua hook rather than patching signed PAK files:

- `unlocks = progression | all | custom`;
- explicit car, paint, mod, playlist, boss, tier and event switches;
- a direct-race browser over built-in locations/events using
  `LaunchCustomRace`;
- arbitrary lap count within validated limits;
- FPS/frame pacing and renderer controls;
- named profiles with isolated saves and reversible snapshots;
- LAN/community server routing as an explicit opt-in.

## LAN contract

Local multiplayer is independent of dead Internet services. On one trusted
Wi-Fi/Ethernet subnet, every player runs their own local game instance and the
original LAN path communicates through UDP broadcast on port 47801.

The product gate is stricter than upstream's same-machine test:

- automatic selection of a private, broadcast-capable interface;
- clear handling of Ethernet plus Wi-Fi plus VPN/Tailscale coexistence;
- explicit firewall diagnosis and a reversible setup action;
- unique player identity and save directory per machine;
- host discovery without manually typing addresses;
- host/join/start/finish, disconnect, refresh and reconnect tests;
- no account, cloud, public relay, port forwarding or Internet dependency.

`blur lan` should perform the diagnostics, start `BLUR_NET_MESH=1`, and explain
only actionable local failures. `blur play` remains loopback-only by default.

Unlocks must be runtime policy, not a foreign 100% save that destroys personal
progress. The default profile remains authentic and untouched.

## Portability contract

- Game data stays on the external source volume.
- A tiny overlay selects the verified executable and plugins.
- Generated lifted code, shader packs and saves stay machine-local and private.
- Config is portable; caches are reproducible; saves are backed up before every
  migration.

## What not to promise yet

- A full custom track editor. The available Blender importer handles models,
  not the complete signed level/physics/navigation pipeline.
- Perfect Windows/browser parity before deterministic frame and input tests.
- Internet matchmaking through the defunct official service. LAN is real;
  community service routing is optional and separately trusted.
- Redistribution of the generated lifted binary: it contains derived game code
  and is strictly local.
