#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

target="wasm32-unknown-unknown"
clang_bin="${CLANG:-clang}"
wasm_ld_bin="${WASM_LD:-wasm-ld}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi
work_dir="$target_dir/validation/wasm-staticlib-consumer"
library="$target_dir/$target/debug/libzirk_runtime.a"

for tool in cargo "$clang_bin" "$wasm_ld_bin"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'required tool not found: %s\n' "$tool" >&2
    printf 'fallback: install the wasm32-unknown-unknown Rust target, Clang with wasm32 support, and wasm-ld; then rerun this script.\n' >&2
    exit 2
  fi
done

rustup target list --installed | rg -qx "$target" || {
  printf 'Rust target is not installed: %s\n' "$target" >&2
  printf 'fallback: rustup target add %s, then rerun this script.\n' "$target" >&2
  exit 2
}

cargo build -p zirk-runtime --lib --target "$target"
[[ -s "$library" ]] || {
  printf 'Cargo did not produce the expected static library: %s\n' "$library" >&2
  exit 1
}

mkdir -p "$work_dir"
cat > "$work_dir/consumer.c" <<'EOF'
extern void zirk_rt_init(void);
extern void zirk_rt_shutdown(void);

__attribute__((export_name("consumer_entry")))
void consumer_entry(void) {
  zirk_rt_init();
  zirk_rt_shutdown();
}
EOF

"$clang_bin" --target="$target" -ffreestanding -fno-builtin \
  -c "$work_dir/consumer.c" -o "$work_dir/consumer.o"
"$wasm_ld_bin" --no-entry --export=consumer_entry --gc-sections \
  "$work_dir/consumer.o" "$library" -o "$work_dir/consumer.wasm"

printf 'Consumer link succeeded (link only; the WebAssembly module was not executed): %s\n' \
  "$work_dir/consumer.wasm"
