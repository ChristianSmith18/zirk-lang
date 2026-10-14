#!/usr/bin/env bash
# Locate the reviewed wasm-opt (binaryen) release the browser package is built with.
set -euo pipefail

readonly required_version="133"

if command -v wasm-opt >/dev/null 2>&1; then
    installed="$(wasm-opt --version | awk '{print $3}')"
    if [[ "$installed" == "$required_version" ]]; then
        exit 0
    fi
    printf 'wasm-opt version_%s is installed; version_%s is required\n' "$installed" "$required_version" >&2
else
    printf 'wasm-opt (binaryen) is not installed\n' >&2
fi
printf 'install binaryen version_%s: brew install binaryen, or the release archive from\n' "$required_version" >&2
printf 'https://github.com/WebAssembly/binaryen/releases/tag/version_%s\n' "$required_version" >&2
exit 1
