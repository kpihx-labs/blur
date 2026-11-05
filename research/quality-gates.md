# Quality gates

Blur is not releasable until every applicable gate below is proven.

| Area | Gate |
|---|---|
| Source | Executable and data hashes are recognized; originals stay immutable |
| Boot | Native Ubuntu runtime reaches the front-end deterministically |
| Gameplay | One complete race starts, runs, finishes, and returns to menus |
| Rendering | WebGL2 baseline is correct; WebGPU comparison has bounded differences |
| Audio | Movies, menu music, race audio and effects run without sustained underruns |
| Input | Keyboard and two common gamepads work; focus loss releases every key |
| Saves | Named profiles are isolated, snapshotted, restorable and cross-platform tested |
| Unlocks | Authentic/all/custom policies are reversible and never corrupt progression |
| Direct race | Built-in tracks/events can be selected without fake progression edits |
| LAN | Two physical same-subnet PCs discover, host, join, race, disconnect and reconnect |
| Network isolation | LAN works with Internet unavailable and without VPN/relay/account |
| Performance | Frame pacing, memory, object counts and long-session stability are bounded |
| Windows | Verified original executable plus hook matches the shared product contract |
| Packaging | No proprietary game byte or generated lifted binary enters Git/releases |

## Cross-game parity

Where technically applicable, Blur also receives the collection-wide contract:
named profiles, authentic/free presets, save snapshots, deterministic scripted
replays, controller diagnostics, direct content selection, LAN discovery and
version/hash handshakes.
