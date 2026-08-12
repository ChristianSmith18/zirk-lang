#!/usr/bin/env bash
#
# Verificación local del workspace.
#
# Ejecuta exactamente lo mismo que CI. En macOS es *la* verificación: macOS no
# está en la matriz de CI y se cubre desde la máquina de desarrollo.
# Ver docs/decisions/ADR-004-portabilidad.md
#
# Uso:  ./scripts/check-local.sh

set -euo pipefail

cd "$(dirname "$0")/.."

azul()  { printf '\033[1;34m%s\033[0m\n' "$1"; }
verde() { printf '\033[1;32m%s\033[0m\n' "$1"; }
rojo()  { printf '\033[1;31m%s\033[0m\n' "$1"; }

# --- Toolchain ---------------------------------------------------------------

if [[ -z "${LLVM_SYS_201_PREFIX:-}" ]]; then
    if command -v brew >/dev/null 2>&1 && brew --prefix llvm@20 >/dev/null 2>&1; then
        LLVM_SYS_201_PREFIX="$(brew --prefix llvm@20)"
        export LLVM_SYS_201_PREFIX
        azul "LLVM_SYS_201_PREFIX detectado: $LLVM_SYS_201_PREFIX"
    else
        rojo "falta LLVM_SYS_201_PREFIX y no se pudo detectar llvm@20"
        echo "  ver docs/TOOLCHAIN.md"
        exit 1
    fi
fi

llvm_version="$("$LLVM_SYS_201_PREFIX/bin/llvm-config" --version)"
if [[ "$llvm_version" != 20.* ]]; then
    rojo "versión de LLVM incompatible: $llvm_version (se requiere 20.1.x)"
    echo "  ver docs/decisions/ADR-001-pin-llvm.md"
    exit 1
fi

azul "== entorno =="
echo "  host:  $(uname -sm)"
echo "  rust:  $(rustc --version)"
echo "  llvm:  $llvm_version"
echo

# --- Verificación ------------------------------------------------------------

azul "== formato =="
cargo fmt --all --check
verde "  ok"
echo

azul "== clippy =="
cargo clippy --workspace --all-targets -- -D warnings
verde "  ok"
echo

azul "== tests =="
cargo test --workspace
echo

verde "== todo en verde en $(uname -sm) =="
