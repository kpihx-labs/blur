#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env.sh
source "$ROOT/scripts/env.sh"

RUNTIME_ROOT="${BLUR_NATIVE_ROOT:-$ROOT/vendor/blur-web}"
[[ -x "$RUNTIME_ROOT/setup.sh" ]] || {
    printf 'blur-web runtime source is unavailable at %s\n' "$RUNTIME_ROOT" >&2
    exit 1
}

exec "$RUNTIME_ROOT/setup.sh" \
    "$ROOT/work/game-overlay" \
    --crt "$ROOT/work/tools/vcredist_x86-9.0.21022.8.exe" \
    --skip-deps "$@"
