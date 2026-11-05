#!/usr/bin/env bash

PROJECT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TOOLS_ROOT="$PROJECT_ROOT/work/tools/root/usr"
export PATH="$TOOLS_ROOT/bin:$PATH"
export CPATH="$TOOLS_ROOT/include${CPATH:+:$CPATH}"
export LIBRARY_PATH="$TOOLS_ROOT/lib/x86_64-linux-gnu${LIBRARY_PATH:+:$LIBRARY_PATH}"
export LD_LIBRARY_PATH="$TOOLS_ROOT/lib/x86_64-linux-gnu${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export PKG_CONFIG_PATH="$TOOLS_ROOT/lib/x86_64-linux-gnu/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
