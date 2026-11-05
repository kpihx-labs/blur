#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env.sh
source "$ROOT/scripts/env.sh"

exec "$ROOT/refs/blur-web/setup.sh" \
    "$ROOT/work/game-overlay" \
    --crt "$ROOT/work/tools/vcredist_x86-9.0.21022.8.exe" \
    --skip-deps "$@"
