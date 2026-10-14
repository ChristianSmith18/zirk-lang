#!/usr/bin/env bash
set -euo pipefail

# Compare target-lowered C declarations against the runtime's Rust LLVM
# definitions, then link a Wasm consumer when those tools/target are available.
# Linking and signature comparison do not execute the consumer.

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

clang_bin="${CLANG:-clang}"
wasm_ld_bin="${WASM_LD:-wasm-ld}"
python_bin="${PYTHON:-python3}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then target_dir="$repo_root/$target_dir"; fi
work_dir="$target_dir/validation/concurrency-abi-signatures"
mkdir -p "$work_dir"

for tool in cargo rustup "$clang_bin" "$python_bin"; do
  command -v "$tool" >/dev/null 2>&1 || { printf 'required tool not found: %s\n' "$tool" >&2; exit 2; }
done

if [[ -n "${ZIRK_ABI_TARGETS:-}" ]]; then
  read -r -a targets <<< "$ZIRK_ABI_TARGETS"
else
  targets=()
  while IFS= read -r target; do targets+=("$target"); done < <(rustup target list --installed | sort)
fi

symbols_expected=30
consumer_source="$repo_root/scripts/concurrency-abi-consumer.c"
checker="$repo_root/scripts/check-concurrency-abi-signatures.py"
for target in "${targets[@]}"; do
  rustup target list --installed | grep -Fqx "$target" || {
    printf 'Rust target is not installed: %s\n' "$target" >&2
    exit 2
  }
  cargo rustc -p zirk-runtime --lib --target "$target" -- --emit=llvm-ir
  library="$target_dir/$target/debug/libzirk_runtime.a"

  llvm_dir="$target_dir/$target/debug/deps"
  rust_llvm="$("$python_bin" -c 'import pathlib,sys; p=pathlib.Path(sys.argv[1]); xs=list(p.glob("zirk_runtime-*.ll")); print(max(xs,key=lambda x:x.stat().st_mtime) if xs else "")' "$llvm_dir")"
  [[ -n "$rust_llvm" && -s "$rust_llvm" ]] || { printf 'missing Rust LLVM IR for %s\n' "$target" >&2; exit 1; }

  consumer_llvm="$work_dir/consumer-$target.ll"
  consumer_object="$work_dir/consumer-$target.o"
  "$clang_bin" --target="$target" -ffreestanding -fno-builtin -O0 -S -emit-llvm \
    "$consumer_source" -o "$consumer_llvm"
  "$python_bin" "$checker" --target "$target" --rust-llvm "$rust_llvm" --consumer-llvm "$consumer_llvm"
  "$clang_bin" --target="$target" -ffreestanding -fno-builtin -c \
    "$consumer_source" -o "$consumer_object"

  case "$target" in
    wasm32-unknown-unknown)
      command -v "$wasm_ld_bin" >/dev/null 2>&1 || { printf 'required tool not found: %s\n' "$wasm_ld_bin" >&2; exit 2; }
      [[ -s "$library" ]] || { printf 'missing Wasm staticlib: %s\n' "$library" >&2; exit 1; }
      "$wasm_ld_bin" --no-entry --export=concurrency_abi_consumer --gc-sections \
        "$consumer_object" "$library" -o "$work_dir/consumer-$target.wasm"
      printf 'linked %s: %s (%s symbols, --no-undefined; not executed)\n' \
        "$target" "$work_dir/consumer-$target.wasm" "$symbols_expected"
      ;;
    *)
      printf 'signatures verified for %s; no consumer link claimed (no configured linker/sysroot contract)\n' "$target"
      ;;
  esac
done
