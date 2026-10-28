#!/usr/bin/env bash
# Build the browser interpreter package from the caller's checked-out source.
#
# Usage: ./scripts/build-browser-package.sh [output-directory]
set -euo pipefail

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
readonly output="${1:-$root/dist/zirk-browser}"
readonly target="wasm32-unknown-unknown"
readonly wasm="$root/target/$target/release/zirk_browser.wasm"

cd "$root"
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
    printf 'refusing to build a provenance package from a dirty revision\n' >&2
    exit 1
fi
readonly revision="$(git rev-parse HEAD)"
./scripts/ensure-wasm-bindgen.sh
./scripts/ensure-wasm-opt.sh
rustup target add "$target"
CARGO_PROFILE_RELEASE_OPT_LEVEL=z cargo build --locked --release -p zirk-browser --no-default-features --target "$target"

if [[ -e "$output" ]]; then
    printf 'refusing to overwrite existing output directory: %s\n' "$output" >&2
    exit 1
fi
mkdir -p "$output"
wasm-bindgen --target web --typescript --out-dir "$output" --out-name zirk_browser "$wasm"

# Size optimization must not change behavior. Capture the observable behavior
# of the unoptimized binary, optimize in place, and require an identical digest
# over every conformance fixture and valid corpus program.
unoptimized="$(mktemp)"
trap 'rm -f "$unoptimized"' EXIT
cp "$output/zirk_browser_bg.wasm" "$unoptimized"
before="$(node ./scripts/verify-browser-package-behavior.mjs "$output" "$unoptimized")"
wasm-opt -Oz \
    --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
    --enable-mutable-globals --enable-reference-types --enable-multivalue \
    "$unoptimized" -o "$output/zirk_browser_bg.wasm"
after="$(node ./scripts/verify-browser-package-behavior.mjs "$output")"
if [[ "$before" != "$after" ]]; then
    printf 'wasm-opt changed observable behavior\nbefore: %s\nafter:  %s\n' "$before" "$after" >&2
    exit 1
fi

cp LICENSE "$output/LICENSE"
cp crates/zirk-browser/browser-api.d.ts "$output/browser-api.d.ts"
node ./scripts/write-browser-manifest.mjs "$output" "$revision"

printf 'browser package written to %s\n' "$output"
