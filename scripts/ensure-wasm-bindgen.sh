#!/usr/bin/env bash
# Locate the reviewed wasm-bindgen CLI, or install exactly that release.
set -euo pipefail

readonly required_version="0.2.128"

if command -v wasm-bindgen >/dev/null 2>&1; then
    installed="$(wasm-bindgen --version | awk '{print $2}')"
    if [[ "$installed" == "$required_version" ]]; then
        exit 0
    fi
    printf 'wasm-bindgen %s is installed; %s is required\n' "$installed" "$required_version" >&2
fi

cargo install wasm-bindgen-cli --version "$required_version" --locked
wasm-bindgen --version | grep -Fqx "wasm-bindgen $required_version"
