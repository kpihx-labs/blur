# TODO

- Fix the post-intro transition and complete one native Ubuntu race.
- Add deterministic trace comparison around account/save initialization.
- Wire the implemented reversible unlock policy metadata to runtime gameplay hooks; isolated profile paths are implemented.
- Implement direct built-in race selection through native Lua functions.
- Validate the zero-service compatibility discovery and original LAN transport across physical Ubuntu and Windows PCs.
- Validate UDP 47801 firewall policy, port conflicts, disconnects, and reconnects; diagnostics never modify firewall state.
- Validate Windows original-executable and hook parity; the current root Wine fallback is Unix-host-only by design.
## Implemented; physical validation remains

- The configurable input schema, CLI, browser/native application, explicit Wine
  refusal boundary, conflict checks, atomic portability, optional presets, and
  LAN behavior fingerprint are implemented. Validate real controller IDs,
  reconnects, and steering feel on physical Ubuntu/Windows hardware; blur-web
  still exposes one active XInput seat and explicitly refuses enabled extra seats.
- `blur perf` now activates existing blur-web host/renderer instrumentation plus
  browser memory and browser-observed-input-change to received-Present-batch
  approximation. Record release-gate baselines in a moving menu/race on target
  hardware; WebGPU GPU duration and display-photon latency remain unavailable
  and are not claimed.
