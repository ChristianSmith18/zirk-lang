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
work_dir="$target_dir/validation/concurrency-abi-consumer"
library="$target_dir/$target/debug/libzirk_runtime.a"

for tool in cargo rustup "$clang_bin" "$wasm_ld_bin"; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    printf 'required tool not found: %s\n' "$tool" >&2
    exit 2
  fi
done

rustup target list --installed | rg -qx "$target" || {
  printf 'Rust target is not installed: %s\n' "$target" >&2
  exit 2
}

cargo build -p zirk-runtime --lib --target "$target"
[[ -s "$library" ]] || {
  printf 'Cargo did not produce the expected static library: %s\n' "$library" >&2
  exit 1
}

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

/* Keep references live so --no-undefined proves each selected C ABI symbol
 * exists in the target static library. This consumer is linked, never run. */
__attribute__((export_name("concurrency_abi_consumer")))
uintptr_t concurrency_abi_consumer(void) {
  void (*const runtime_lifecycle[])(void) = { zirk_rt_init, zirk_rt_shutdown };
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

  /* The address set forces link-time symbol resolution without invoking ABI
   * functions outside their required executor/task context. */
  const void *symbols[] = {
      (const void *)runtime_lifecycle[0], (const void *)runtime_lifecycle[1],
      (const void *)run_main_cli_fn, (const void *)run_main_policy_fn,
      (const void *)request_host_shutdown_fn,
      (const void *)spawn_fn, (const void *)wait_any_fn, (const void *)wait_any_timer_fn,
      (const void *)take_fn,
      (const void *)wait_take_fn, (const void *)zirk_rt_cancel,
      (const void *)zirk_rt_cancel_with_cleanup_failure, (const void *)scope_fn,
      (const void *)collect_scope_fn, (const void *)scope_exit_fn,
      (const void *)collect_scope_exit_fn, (const void *)zirk_rt_push_frame,
      (const void *)zirk_rt_pop_frame, (const void *)zirk_rt_cancel_shield_enter,
      (const void *)zirk_rt_cancel_shield_exit, (const void *)atomic_new_fn,
      (const void *)zirk_rt_atomic_free, (const void *)zirk_rt_atomic_load,
      (const void *)zirk_rt_atomic_store, (const void *)zirk_rt_atomic_exchange,
      (const void *)atomic_compare_fn, (const void *)zirk_rt_atomic_fetch_add,
      (const void *)zirk_rt_atomic_fetch_sub,
  };
  uintptr_t checksum = sizeof(symbols) / sizeof(symbols[0]);
  for (size_t i = 0; i < sizeof(symbols) / sizeof(symbols[0]); ++i)
    checksum += (uintptr_t)symbols[i] != 0;
  return checksum;
}
EOF

"$clang_bin" --target="$target" -ffreestanding -fno-builtin \
  -c "$work_dir/consumer.c" -o "$work_dir/consumer.o"
"$wasm_ld_bin" --no-entry --export=concurrency_abi_consumer --gc-sections \
  "$work_dir/consumer.o" "$library" -o "$work_dir/consumer.wasm"

printf 'Concurrency ABI consumer linked successfully (28 symbols; link only, module not executed): %s\n' \
  "$work_dir/consumer.wasm"
