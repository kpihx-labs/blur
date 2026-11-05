#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=env.sh
source "$ROOT/scripts/env.sh"

check_hash() {
    local expected="$1" file="$2" actual
    actual="$(sha256sum "$file" | cut -d' ' -f1)"
    [[ "$actual" == "$expected" ]] || {
        printf 'hash mismatch: %s\n' "$file" >&2
        return 1
    }
}

check_hash 7ef92b57bfd8741cf3de44225e3586c99d7365d7e212b86b3229fbb7d99deee9 \
    "$ROOT/work/game-overlay/Blur.exe"
check_hash c6a7e484f4d84883bc1205bccea3114c0521025712922298ede9b2a1cd632357 \
    "$ROOT/work/tools/vcredist_x86-9.0.21022.8.exe"
[[ -x "$ROOT/work/tools/root/usr/bin/ld.lld" ]]
[[ -f "$ROOT/refs/blur-web/port/vendor/msvcr90.dll" ]]
[[ -x "$ROOT/refs/blur-web/port/harness/blurrt_interp" ]]
[[ -x "$ROOT/refs/blur-web/port/harness/blurrt" ]]
[[ -f "$ROOT/work/native-lift-smoke.png" ]]

printf 'blur tools, interpreter and native lift: ok\n'
