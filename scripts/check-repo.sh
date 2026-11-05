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
[[ -z "$changes" ]] || {
    printf '%s has local changes:\n%s\n' "$SUBMODULE" "$changes" >&2
    exit 1
}

[[ -x "$ROOT/$SUBMODULE/setup.sh" ]]
[[ -x "$ROOT/$SUBMODULE/play.sh" ]]
printf 'blur repository and pinned runtime source: ok\n'
