#!/usr/bin/env bash
# Builds the extension and runs tests/godot/test.gd in a headless Godot.
#   GODOT=/path/to/godot tests/run.sh
set -euo pipefail
cd "$(dirname "$0")/.."
GODOT="${GODOT:-godot}"
cargo build ${CARGO_ARGS:-}
mkdir -p tests/godot/libs
cp target/debug/libgodot_opus.so tests/godot/libs/
"$GODOT" --headless --path tests/godot --import >/dev/null 2>&1 || true
OUT=$("$GODOT" --headless --path tests/godot -s res://test.gd 2>&1) || STATUS=$?
grep -E "^(ok|FAIL|ALL PASSED|FAILED)|ERROR" <<<"$OUT" || true
grep -q "^ALL PASSED" <<<"$OUT" && [ "${STATUS:-0}" -eq 0 ]
