#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env.sh
source "$ROOT/scripts/env.sh"
RUNTIME_ROOT="${BLUR_NATIVE_ROOT:-$ROOT/vendor/blur-web}"
if [[ ! -x "$RUNTIME_ROOT/play.sh" && -x "$ROOT/refs/blur-web/play.sh" ]]; then
    RUNTIME_ROOT="$ROOT/refs/blur-web"
fi
[[ -x "$RUNTIME_ROOT/play.sh" ]] || {
    printf 'built blur-web runtime is unavailable; run make setup first\n' >&2
    exit 1
}
cd "$RUNTIME_ROOT"
exec ./play.sh "$@"
