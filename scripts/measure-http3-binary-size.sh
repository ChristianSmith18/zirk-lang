#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(git rev-parse --show-toplevel)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/zirk-http3-size.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

cd "$ROOT_DIR"
cargo build --release -p zirk-cli
cargo build --release -p zirk-runtime

mkdir -p "$WORK_DIR/base" "$WORK_DIR/http3"
cp target/release/zirk target/release/libzirk_runtime.a target/release/libzirk_runtime.rlib "$WORK_DIR/base/"
cp scripts/fixtures/http3-size/main.zrk "$WORK_DIR/base/"
(cd "$WORK_DIR/base" && ./zirk build main.zrk >/dev/null)

cargo build --release -p zirk-runtime --features http3
cp target/release/zirk target/release/libzirk_runtime.a target/release/libzirk_runtime.rlib "$WORK_DIR/http3/"
cp scripts/fixtures/http3-size/main.zrk "$WORK_DIR/http3/"
(cd "$WORK_DIR/http3" && ./zirk build main.zrk >/dev/null)

python3 - \
    "$WORK_DIR/base/build/main" \
    "$WORK_DIR/http3/build/main" \
    "$WORK_DIR/base/libzirk_runtime.a" \
    "$WORK_DIR/http3/libzirk_runtime.a" \
    "$WORK_DIR/base/libzirk_runtime.rlib" \
    "$WORK_DIR/http3/libzirk_runtime.rlib" <<'PY'
from pathlib import Path
import sys

base = Path(sys.argv[1]).stat().st_size
http3 = Path(sys.argv[2]).stat().st_size
increase = http3 - base
print(f"Release executable without HTTP/3: {base:,} bytes ({base / 1_048_576:.2f} MiB)")
print(f"Release executable with HTTP/3:    {http3:,} bytes ({http3 / 1_048_576:.2f} MiB)")
print(f"HTTP/3 linked size increase:       {increase:,} bytes ({increase / 1_048_576:.2f} MiB, {increase / base * 100:.1f}%)")

base_archive = Path(sys.argv[3]).stat().st_size
http3_archive = Path(sys.argv[4]).stat().st_size
archive_increase = http3_archive - base_archive
print(f"Runtime archive without HTTP/3:    {base_archive:,} bytes ({base_archive / 1_048_576:.2f} MiB)")
print(f"Runtime archive with HTTP/3:       {http3_archive:,} bytes ({http3_archive / 1_048_576:.2f} MiB)")
print(f"HTTP/3 archive size increase:      {archive_increase:,} bytes ({archive_increase / 1_048_576:.2f} MiB, {archive_increase / base_archive * 100:.1f}%)")

base_rlib = Path(sys.argv[5]).stat().st_size
http3_rlib = Path(sys.argv[6]).stat().st_size
rlib_increase = http3_rlib - base_rlib
print(f"Runtime rlib without HTTP/3:       {base_rlib:,} bytes ({base_rlib / 1_048_576:.2f} MiB)")
print(f"Runtime rlib with HTTP/3:          {http3_rlib:,} bytes ({http3_rlib / 1_048_576:.2f} MiB)")
print(f"HTTP/3 rlib size increase:         {rlib_increase:,} bytes ({rlib_increase / 1_048_576:.2f} MiB, {rlib_increase / base_rlib * 100:.1f}%)")
PY
