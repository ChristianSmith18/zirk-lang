#!/usr/bin/env bash
set -euo pipefail

# Link a freestanding C reference consumer against the Linux Rust staticlib.
# This validates symbol availability and final-link compatibility. C declarations
# express the intended signatures, but linking cannot verify C ABI signatures.
# It does not execute the consumer or prove runtime behavior. A real Linux
# sysroot is required.
#
# Optional environment:
#   ZIRK_LINUX_SYSROOT  Linux sysroot (defaults to /)
#   ZIRK_LINUX_TARGETS  whitespace-separated Rust targets (defaults to both)
#   ZIRK_USE_PREBUILT_STATICLIBS=1  skip Cargo/rustup and link existing archives
#   CLANG               clang executable (defaults to clang)
#   CARGO_TARGET_DIR    Cargo output directory (defaults to <repo>/target)

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

clang_bin="${CLANG:-clang}"
sysroot="${ZIRK_LINUX_SYSROOT:-/}"
targets="${ZIRK_LINUX_TARGETS:-x86_64-unknown-linux-gnu i686-unknown-linux-gnu}"
use_prebuilt="${ZIRK_USE_PREBUILT_STATICLIBS:-0}"
target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
if [[ "$target_dir" != /* ]]; then
  target_dir="$repo_root/$target_dir"
fi

tools=("$clang_bin")
tools+=(ld.lld)
if [[ "$use_prebuilt" != 1 ]]; then
  tools+=(cargo rustup grep)
fi
for tool in "${tools[@]}"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'required tool not found: %s\n' "$tool" >&2
    exit 2
  fi
done
if [[ ! -d "$sysroot" ]]; then
  printf 'Linux sysroot directory not found: %s\n' "$sysroot" >&2
  exit 2
fi

work_dir="$target_dir/validation/concurrency-abi-consumer-linux"
mkdir -p "$work_dir"
cat > "$work_dir/consumer.c" <<'EOF'
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct AtomicCell AtomicCell;
typedef void (*ZirkMain)(void);

extern void zirk_rt_init(void);
extern void zirk_rt_shutdown(void);
extern void zirk_rt_run_main_cli(ZirkMain);
extern void zirk_rt_run_main_with_deadline_policy(ZirkMain, uint32_t);
extern uint32_t zirk_rt_request_host_shutdown(int32_t);
extern uint64_t zirk_rt_spawn(void (*)(void *), void *);
extern uint64_t zirk_rt_job_wait_any(const uint64_t *, uint64_t);
extern uint64_t zirk_rt_job_wait_any_with_timer(const uint64_t *, uint64_t);
extern uint32_t zirk_rt_job_outcome_take(uint64_t, void **, void **);
extern uint32_t zirk_rt_job_outcome_wait_take(uint64_t, void **, void **);
extern void zirk_rt_cancel(uint64_t);
extern void zirk_rt_cancel_with_cleanup_failure(uint64_t);
extern uint64_t zirk_rt_scope_enter(void);
extern uint64_t zirk_rt_scope_enter_collect_all(void);
extern bool zirk_rt_scope_exit(uint64_t);
extern bool zirk_rt_scope_exit_collect_all(uint64_t);
extern void zirk_rt_push_frame(void **, int64_t);
extern void zirk_rt_pop_frame(void);
extern void zirk_rt_cancel_shield_enter(void);
extern void zirk_rt_cancel_shield_exit(void);
extern int32_t zirk_rt_atomic_new(uint32_t, uint64_t, AtomicCell **);
extern void zirk_rt_atomic_free(AtomicCell *);
extern int32_t zirk_rt_atomic_load(const AtomicCell *, uint64_t *);
extern int32_t zirk_rt_atomic_store(const AtomicCell *, uint64_t);
extern int32_t zirk_rt_atomic_exchange(const AtomicCell *, uint64_t, uint64_t *);
extern int32_t zirk_rt_atomic_compare_exchange(
    const AtomicCell *, uint64_t, uint64_t, uint32_t, uint32_t, uint64_t *, bool *);
extern int32_t zirk_rt_atomic_fetch_add(const AtomicCell *, uint64_t, uint64_t *);
extern int32_t zirk_rt_atomic_fetch_sub(const AtomicCell *, uint64_t, uint64_t *);

/* Keep references live so the final executable link resolves every symbol.
 * It is intentionally never run: many functions require executor/task state. */
__attribute__((used, noinline))
int main(void) {
  void (*const lifecycle[])(void) = { zirk_rt_init, zirk_rt_shutdown };
  void (*const run_main_cli_fn)(ZirkMain) = zirk_rt_run_main_cli;
  void (*const run_main_policy_fn)(ZirkMain, uint32_t) = zirk_rt_run_main_with_deadline_policy;
  uint32_t (*const request_host_shutdown_fn)(int32_t) = zirk_rt_request_host_shutdown;
  uint64_t (*const spawn_fn)(void (*)(void *), void *) = zirk_rt_spawn;
  uint64_t (*const wait_any_fn)(const uint64_t *, uint64_t) = zirk_rt_job_wait_any;
  uint64_t (*const wait_any_timer_fn)(const uint64_t *, uint64_t) = zirk_rt_job_wait_any_with_timer;
  uint32_t (*const take_fn)(uint64_t, void **, void **) = zirk_rt_job_outcome_take;
  uint32_t (*const wait_take_fn)(uint64_t, void **, void **) = zirk_rt_job_outcome_wait_take;
  uint64_t (*const scope_fn)(void) = zirk_rt_scope_enter;
  uint64_t (*const collect_scope_fn)(void) = zirk_rt_scope_enter_collect_all;
  bool (*const scope_exit_fn)(uint64_t) = zirk_rt_scope_exit;
  bool (*const collect_scope_exit_fn)(uint64_t) = zirk_rt_scope_exit_collect_all;
  int32_t (*const atomic_new_fn)(uint32_t, uint64_t, AtomicCell **) = zirk_rt_atomic_new;
  int32_t (*const atomic_compare_fn)(const AtomicCell *, uint64_t, uint64_t,
                                     uint32_t, uint32_t, uint64_t *, bool *) =
      zirk_rt_atomic_compare_exchange;
  const void *const symbols[] = {
      (const void *)lifecycle[0], (const void *)lifecycle[1],
      (const void *)run_main_cli_fn, (const void *)run_main_policy_fn,
      (const void *)request_host_shutdown_fn, (const void *)spawn_fn,
      (const void *)wait_any_fn, (const void *)wait_any_timer_fn, (const void *)take_fn,
      (const void *)wait_take_fn,
      (const void *)zirk_rt_cancel, (const void *)zirk_rt_cancel_with_cleanup_failure,
      (const void *)scope_fn, (const void *)collect_scope_fn, (const void *)scope_exit_fn,
      (const void *)collect_scope_exit_fn, (const void *)zirk_rt_push_frame,
      (const void *)zirk_rt_pop_frame, (const void *)zirk_rt_cancel_shield_enter,
      (const void *)zirk_rt_cancel_shield_exit, (const void *)atomic_new_fn,
      (const void *)zirk_rt_atomic_free, (const void *)zirk_rt_atomic_load,
      (const void *)zirk_rt_atomic_store, (const void *)zirk_rt_atomic_exchange,
      (const void *)atomic_compare_fn, (const void *)zirk_rt_atomic_fetch_add,
      (const void *)zirk_rt_atomic_fetch_sub,
  };
  uintptr_t count = 0;
  for (size_t i = 0; i < sizeof(symbols) / sizeof(symbols[0]); ++i)
    count += symbols[i] != NULL;
  return (int)count;
}
EOF

if [[ "$use_prebuilt" != 1 ]]; then
  rustup target list --installed > "$work_dir/installed-rust-targets.txt"
fi
read -r -a target_list <<< "$targets"
for target in "${target_list[@]}"; do
  case "$target" in
    x86_64-unknown-linux-gnu) elf_machine="x86-64" ;;
    i686-unknown-linux-gnu) elf_machine="Intel 80386" ;;
    *)
      printf 'unsupported target for this consumer: %s\n' "$target" >&2
      exit 2
      ;;
  esac

  if [[ "$use_prebuilt" != 1 ]] && ! grep -Fqx "$target" "$work_dir/installed-rust-targets.txt"; then
    printf 'Rust target is not installed: %s\n' "$target" >&2
    exit 2
  fi

  crt="$($clang_bin --target="$target" --sysroot="$sysroot" -print-file-name=crt1.o)"
  if [[ "$crt" == crt1.o || ! -s "$crt" ]]; then
    printf 'sysroot lacks target startup objects for %s: %s\n' "$target" "$sysroot" >&2
    exit 2
  fi

  if [[ "$use_prebuilt" != 1 ]]; then
    cargo build -p zirk-runtime --lib --target "$target"
  fi
  library="$target_dir/$target/debug/libzirk_runtime.a"
  if [[ ! -s "$library" ]]; then
    printf 'Cargo did not produce the expected static library: %s\n' "$library" >&2
    exit 1
  fi

  consumer_object="$work_dir/consumer-$target.o"
  consumer_executable="$work_dir/consumer-$target"
  "$clang_bin" --target="$target" --sysroot="$sysroot" -ffreestanding \
    -fno-builtin -fno-pie -c "$work_dir/consumer.c" -o "$consumer_object"
  "$clang_bin" --target="$target" --sysroot="$sysroot" -fuse-ld=lld \
    -no-pie "$consumer_object" "$library" -Wl,--no-undefined \
    -lpthread -ldl -lm -lrt -lutil -o "$consumer_executable"

  if command -v file >/dev/null 2>&1; then
    description="$(file -b "$consumer_executable")"
    if [[ "$description" != *"ELF"* || "$description" != *"$elf_machine"* ]]; then
      printf 'unexpected linked output for %s: %s\n' "$target" "$description" >&2
      exit 1
    fi
    printf 'linked %s: %s (%s; 28 ABI symbols; --no-undefined; link only)\n' \
      "$target" "$consumer_executable" "$description"
  else
    printf 'linked %s: %s (28 ABI symbols; --no-undefined; link only)\n' \
      "$target" "$consumer_executable"
  fi
done
