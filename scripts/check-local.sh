#!/usr/bin/env bash
#
# Local verification of the workspace.
#
# Runs exactly what CI runs. Use it before opening a pull request.
# See docs/decisions/ADR-004-portabilidad.md
#
# Usage:  ./scripts/check-local.sh

set -euo pipefail

cd "$(dirname "$0")/.."

blue()  { printf '\033[1;34m%s\033[0m\n' "$1"; }
green() { printf '\033[1;32m%s\033[0m\n' "$1"; }
red()   { printf '\033[1;31m%s\033[0m\n' "$1"; }

# --- Toolchain ---------------------------------------------------------------

if [[ -z "${LLVM_SYS_201_PREFIX:-}" ]]; then
    if command -v brew >/dev/null 2>&1 && brew --prefix llvm@20 >/dev/null 2>&1; then
        LLVM_SYS_201_PREFIX="$(brew --prefix llvm@20)"
        export LLVM_SYS_201_PREFIX
        blue "LLVM_SYS_201_PREFIX detected: $LLVM_SYS_201_PREFIX"
    else
        red "LLVM_SYS_201_PREFIX is unset and llvm@20 could not be detected"
        echo "  see docs/TOOLCHAIN.md"
        exit 1
    fi
fi

llvm_version="$("$LLVM_SYS_201_PREFIX/bin/llvm-config" --version)"
if [[ "$llvm_version" != 20.* ]]; then
    red "incompatible LLVM version: $llvm_version (20.1.x is required)"
    echo "  see docs/decisions/ADR-001-pin-llvm.md"
    exit 1
fi

blue "== environment =="
echo "  host:  $(uname -sm)"
echo "  rust:  $(rustc --version)"
echo "  llvm:  $llvm_version"
echo

# --- Verification ------------------------------------------------------------

blue "== formatting =="
cargo fmt --all --check
green "  ok"
echo

blue "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings
green "  ok"
echo

blue "== tests =="
cargo test --workspace -- --test-threads=1
echo

green "== all green on $(uname -sm) =="
