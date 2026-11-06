# TODO

- Fix the post-intro transition and complete one native Ubuntu race.
- Trace the native title/loading state after the verified host bootstrap key:
  Intel Arc/WebGL2 sustains 60 FPS at 18 draws with zero skips, but no Lua
  binding is reached and the spinner remains. The interpreter is too slow to
  render a frame within the bounded 120-second diagnostic.
- Add deterministic trace comparison around account/save initialization.
- Wire the implemented reversible unlock policy metadata to runtime gameplay hooks; isolated profile paths are implemented.
- Implement direct built-in race selection through native Lua functions.
- Validate the zero-service compatibility discovery and original LAN transport across physical Ubuntu and Windows PCs.
- Validate UDP 47801 firewall policy, port conflicts, disconnects, and reconnects; diagnostics never modify firewall state.
- Validate Windows original-executable and hook parity; the current root Wine fallback is Unix-host-only by design.
- Verify the owned game's packed PAK localization inventory without modifying
  it; current filename-only detection intentionally does not claim complete
  French game localization from opaque packed assets.
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
- Heavy setup now defaults to four jobs after a documented 22-worker OOM. Race
  smokes preflight Puppeteer, use disposable profiles, bound host cleanup, and
  are executed inside explicit RAM/swap/CPU user scopes.
- Versioned per-profile language selection, extensible launcher catalogs,
  English/French coverage, strict English fallback, native and Wine propagation,
  localized browser wrapper/HUD, atomic CLI portability, and LAN language
  fingerprinting are implemented. Validate French text/audio physically in a
  complete race before calling the proprietary game localization complete.
