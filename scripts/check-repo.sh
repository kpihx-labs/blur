#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SUBMODULE="vendor/blur-web"

expected="$(git -C "$ROOT" rev-parse ":$SUBMODULE")"
[[ -d "$ROOT/$SUBMODULE/.git" || -f "$ROOT/$SUBMODULE/.git" ]] || {
    printf '%s is not initialized; run git submodule update --init --recursive\n' "$SUBMODULE" >&2
    exit 1
}
actual="$(git -C "$ROOT/$SUBMODULE" rev-parse HEAD)"
[[ "$actual" == "$expected" ]] || {
    printf '%s mismatch: expected %s, found %s\n' "$SUBMODULE" "$expected" "$actual" >&2
    exit 1
}
changes="$(git -C "$ROOT/$SUBMODULE" status --porcelain)"
if [[ -n "$changes" ]]; then
    git -C "$ROOT/$SUBMODULE" diff --check
    while IFS= read -r change; do
        path="${change:3}"
        case "$path" in
            Makefile|README.md|play.sh|port/harness/blur_locale.h|port/harness/hle.c|port/harness/hle2.c|port/harness/play.sh|port/harness/audio.c|port/harness/com.c|port/harness/emu.h|port/harness/net.c|port/harness/rtprof.c|port/harness/test_locale.c|port/harness/test_net_safety.py|port/harness/test_sync.c|port/harness/web/hcache.js|port/harness/web/index.html|port/harness/web/input.js|port/harness/web/input.test.js|port/harness/web/locale.js|port/harness/web/locale.test.js|port/harness/web/renderer.js|port/harness/web/renderer_gpu.js) ;;
            *) printf '%s has an unexpected local change: %s\n' "$SUBMODULE" "$change" >&2; exit 1;;
        esac
    done <<<"$changes"
    printf '%s has only the reviewed integration worktree paths; its pinned HEAD is unchanged\n' "$SUBMODULE"
fi

[[ -x "$ROOT/$SUBMODULE/setup.sh" ]]
[[ -x "$ROOT/$SUBMODULE/play.sh" ]]
printf 'blur repository and pinned runtime source: ok\n'
