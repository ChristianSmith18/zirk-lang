#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/zirk-http3-size.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

cd "$ROOT_DIR"
cargo build --release -p zirk-cli
cargo build --release -p zirk-runtime

mkdir -p "$WORK_DIR/base" "$WORK_DIR/http3"
cp target/release/zirk target/release/libzirk_runtime.a "$WORK_DIR/base/"
cp scripts/fixtures/http3-size/main.zrk "$WORK_DIR/base/"
(cd "$WORK_DIR/base" && ./zirk build main.zrk >/dev/null)

cargo build --release -p zirk-runtime --features http3
cp target/release/zirk target/release/libzirk_runtime.a "$WORK_DIR/http3/"
cp scripts/fixtures/http3-size/main.zrk "$WORK_DIR/http3/"
(cd "$WORK_DIR/http3" && ./zirk build main.zrk >/dev/null)

python3 - "$WORK_DIR/base/build/main" "$WORK_DIR/http3/build/main" <<'PY'
from pathlib import Path
import sys

base = Path(sys.argv[1]).stat().st_size
http3 = Path(sys.argv[2]).stat().st_size
increase = http3 - base
print(f"Release executable without HTTP/3: {base:,} bytes ({base / 1_048_576:.2f} MiB)")
print(f"Release executable with HTTP/3:    {http3:,} bytes ({http3 / 1_048_576:.2f} MiB)")
print(f"HTTP/3 linked size increase:       {increase:,} bytes ({increase / 1_048_576:.2f} MiB, {increase / base * 100:.1f}%)")
PY
