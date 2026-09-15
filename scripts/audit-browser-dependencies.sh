#!/usr/bin/env bash
# Proves the browser crate's WASM dependency closure excludes native backends.
set -euo pipefail

readonly root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

tree="$(cargo tree --locked -p zirk-browser --target wasm32-unknown-unknown -e normal)"
for forbidden in zirk-codegen-llvm zirk-runtime inkwell llvm-sys; do
    if grep -Eq "(^|[[:space:]│├└])${forbidden}([[:space:]v]|$)" <<<"$tree"; then
        printf 'browser dependency closure contains forbidden native component: %s\n' "$forbidden" >&2
        exit 1
    fi
done

# The facade may name forbidden IR capabilities only to reject them. It must
# not acquire native runtime, collector, stdout, process, environment, clock,
# random, storage, HTTP, shell, or DOM authority through a direct dependency.
if grep -Eq 'zirk-runtime|zirk-codegen-llvm|inkwell|llvm-sys' crates/zirk-browser/Cargo.toml; then
    printf 'browser crate declares a forbidden native dependency\n' >&2
    exit 1
fi

# Direct ambient authority is forbidden even if it would happen to compile for
# WASM. The interpreted program receives only CapturedHost-style output and
# cancellation, never filesystem, HTTP, environment, process, clock, random,
# storage, or DOM handles.
if rg -n 'std::(fs|env|process|net|time)|web_sys|js_sys|localStorage|sessionStorage' \
    crates/zirk-browser/src --glob '*.rs'; then
    printf 'browser facade directly references an ambient host capability\n' >&2
    exit 1
fi

printf '%s\n' "$tree"
printf 'browser dependency audit passed\n'
