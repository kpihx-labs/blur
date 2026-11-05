# Initial audit

- Installed data size: approximately 7.4 GiB.
- Executable architecture: Windows PE32, Direct3D 9 era.
- Installed executable SHA-256:
  `6832d823...24594` (NoDVD variant; not supported by `blur-web`).
- The local source archive contains executable SHA-256
  `7ef92b57bfd8741cf3de44225e3586c99d7365d7e212b86b3229fbb7d99deee9`,
  exactly the build required by `blur-web`.
- Installed `cache/gamedata.pak` matches the local official 1.2 patch payload.
- The game data exposes 19 level directories, but core resources remain packed
  and signature-coupled.
- `blur-d3d9-hooks` already proves in-process C++/Lua hooks, configurable FPS,
  more laps, paint unlocks and community-network redirection.
- `blur-web` provides a native lifted runtime, browser renderer, audio,
  keyboard/gamepad, saves, LAN and deterministic regression tooling. Its Linux
  path is explicitly best-effort upstream and therefore must be validated here.

`work/game-overlay` occupies only the selected executable physically; all large
asset directories are symlinked read-only from the mounted original.

## Native opening results

- The exact Microsoft VC++ 2008 runtime was recovered as a signed redistributable
  and verified by SHA-256
  `c6a7e484f4d84883bc1205bccea3114c0521025712922298ede9b2a1cd632357`.
- `msvcr90.dll` extracted from it matches blur-web's required hash
  `ed0170d3de86da33e02bfa1605eec8ff6010583481b1c530843867c1939d2185`.
- Unicorn 2.1.4 and VKD3D 2.1 compile locally on Ubuntu.
- The native interpreter host builds successfully.
- The protected executable opens under Unicorn and yields a 29,814,784-byte
  runtime image plus a 667,648-byte MSVCR image for lifting.
- The full lift completed: 373,517 functions were recompiled and linked into the
  fast native `blurrt` host.
- Ubuntu/Edge smoke test: Activision intro rendered through WebGL2, keyboard was
  connected, DirectSound created its 24/48 kHz buffers, audio ran with zero
  underruns/drops, and the host held 60 FPS.
- Current blocker: after the intro, the Linux lifted runtime remains on the game
  loading spinner instead of reaching the front-end menu. The next trace must
  isolate first-run account/save HLE and the intro-to-menu transition.

## Customization surface

The original executable exposes Lua/runtime names for `LaunchCustomRace`,
`SinglePlayer.UnlockAllCars`, `UnlockAllMods`, `UnlockAllChallengePacks`,
`UnlockAllPlaylists`, boss/event/tier unlocks and RaceBook functions. Existing
d3d9 hooks already execute Lua inside the process. Therefore unlock-anything and
direct-race selection should be runtime plugins, not foreign 100% save files or
destructive edits to signed PAK data.
