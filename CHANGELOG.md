# Changelog

## 0.1.0 - Unreleased

- Establish the cross-platform Blur preservation and customization architecture.
- Select and verify the supported executable while keeping the original data external and immutable.
- Build Unicorn, VKD3D, interpreter, and 373,517-function native lift on Ubuntu.
- Validate WebGL2, input, audio, intro rendering, and stable 60 FPS.
- Isolate the current post-intro loading boundary for deterministic debugging.
- Add the Rust 2024 `blur` CLI with doctor, native/Wine launch routing, isolated profiles, and generated overlays.
- Add explicit reversible authentic/all/custom unlock policy metadata without claiming gameplay-hook enforcement.
- Add zero-service UDP LAN discovery with verified runtime compatibility fingerprints, subnet/policy filtering, and compatibility-preflight host/join flow around the original transport.
- Add profile, metadata, LAN protocol/filtering, and CLI help tests plus Rust format, Clippy, test, and release gates.
- Add a versioned per-profile input schema and `show/set/reset/import/export/validate` CLI with atomic replacement, conflict/range checks, optional authentic/modern presets, portable multi-player/device data, and explicit native/Wine applicability failures.
- Integrate profile keyboard/gamepad mappings, device selectors, axes, deadzones, steering curves, and hotkeys into the blur-web browser path; include the mapping hash in LAN behavioral compatibility.
- Add `blur perf`, reusing blur-web host and renderer metrics for frame pacing, CPU, renderer/GPU timing where available, memory, approximate event-to-present latency, and teardown without claiming display latency or unavailable GPU counters.
- Add synthetic input preset, conflict, runtime-applicability, atomic CLI round-trip, and profile-generation tests.
- Harden input imports and browser delivery against unknown fields, symlinks, oversized files, ambiguous selectors, cross-device conflicts, non-atomic Windows replacement, and silent configured-input fallback.
- Align the public LAN fingerprint helper with the on-wire game/runtime/input behavior hash and reject non-hex fingerprints.
- Clarify that browser timing measures received Presents and browser-observed input changes, while hardening C allocation/file handling and adding browser input transformation tests.
