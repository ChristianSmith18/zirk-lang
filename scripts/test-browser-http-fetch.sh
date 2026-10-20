#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
wasm_target="$repo_root/target/wasm32-unknown-unknown/debug/zirk_browser.wasm"
output_dir="$repo_root/target/wasm-http-fetch-test"

cd "$repo_root"
command -v wasm-bindgen >/dev/null || {
  echo "wasm-bindgen is required; install the CLI matching the workspace wasm-bindgen version."
  exit 1
}

rustup target add wasm32-unknown-unknown
cargo build -p zirk-browser --target wasm32-unknown-unknown
wasm-bindgen --target nodejs --out-dir "$output_dir" "$wasm_target"
node crates/zirk-browser/tests/wasm_http_fetch.cjs "$output_dir/zirk_browser.js"
