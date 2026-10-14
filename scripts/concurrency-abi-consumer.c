#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

typedef struct AtomicCell AtomicCell;
typedef void (*ZirkMain)(void);
typedef size_t (*ZirkSpawnCallback)(void *);
typedef void (*ZirkMutexCallback)(void *, void *);

extern void zirk_rt_init(void);
extern void zirk_rt_shutdown(void);
extern void zirk_rt_run_main_cli(ZirkMain);
extern void zirk_rt_run_main_with_deadline_policy(ZirkMain, uint32_t);
extern uint32_t zirk_rt_request_host_shutdown(int32_t);
extern uint64_t zirk_rt_spawn(size_t (*)(void *), void *);
/* LLVM opaque pointers hide this nested prototype in signature comparison.
 * Keep the callback ABI explicit and make every target compile verify it. */
_Static_assert(__builtin_types_compatible_p(__typeof__(&zirk_rt_spawn),
                                           uint64_t (*)(ZirkSpawnCallback, void *)),
               "zirk_rt_spawn callback ABI drift");
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
extern void *zirk_rt_gc_mutex_alloc(const void *, size_t, size_t,
                                    const size_t *, size_t, uint32_t *);
extern uint32_t zirk_rt_gc_mutex_acquire(void *, void **, uint32_t *);
extern uint32_t zirk_rt_gc_mutex_release(void *, uint32_t *);
extern uint32_t zirk_rt_gc_mutex_with(void *, void (*)(void *, void *), void *);
_Static_assert(__builtin_types_compatible_p(
                   __typeof__(&zirk_rt_gc_mutex_with),
                   uint32_t (*)(void *, ZirkMutexCallback, void *)),
               "zirk_rt_gc_mutex_with callback ABI drift");

/* A link-only consumer: keep every address observable without invoking runtime
 * functions whose preconditions require an initialized executor or task. */
__attribute__((used, noinline))
uintptr_t concurrency_abi_consumer(void) {
  void (*const run_main_cli_fn)(ZirkMain) = zirk_rt_run_main_cli;
  void (*const run_main_policy_fn)(ZirkMain, uint32_t) = zirk_rt_run_main_with_deadline_policy;
  const void *volatile symbols[] = {
      (const void *)zirk_rt_init,
      (const void *)zirk_rt_shutdown,
      (const void *)run_main_cli_fn, (const void *)run_main_policy_fn,
      (const void *)zirk_rt_request_host_shutdown,
      (const void *)zirk_rt_spawn,
      (const void *)zirk_rt_job_wait_any,
      (const void *)zirk_rt_job_wait_any_with_timer,
      (const void *)zirk_rt_job_outcome_take,
      (const void *)zirk_rt_job_outcome_wait_take,
      (const void *)zirk_rt_cancel,
      (const void *)zirk_rt_cancel_with_cleanup_failure,
      (const void *)zirk_rt_scope_enter,
      (const void *)zirk_rt_scope_enter_collect_all,
      (const void *)zirk_rt_scope_exit,
      (const void *)zirk_rt_scope_exit_collect_all,
      (const void *)zirk_rt_push_frame,
      (const void *)zirk_rt_pop_frame,
      (const void *)zirk_rt_cancel_shield_enter,
      (const void *)zirk_rt_cancel_shield_exit,
      (const void *)zirk_rt_atomic_new,
      (const void *)zirk_rt_atomic_free,
      (const void *)zirk_rt_atomic_load,
      (const void *)zirk_rt_atomic_store,
      (const void *)zirk_rt_atomic_exchange,
      (const void *)zirk_rt_atomic_compare_exchange,
      (const void *)zirk_rt_atomic_fetch_add,
      (const void *)zirk_rt_atomic_fetch_sub,
      (const void *)zirk_rt_gc_mutex_alloc,
      (const void *)zirk_rt_gc_mutex_acquire,
      (const void *)zirk_rt_gc_mutex_release,
      (const void *)zirk_rt_gc_mutex_with,
  };
  uintptr_t count = 0;
  for (size_t i = 0; i < sizeof(symbols) / sizeof(symbols[0]); ++i)
    count += symbols[i] != NULL;
  return count;
}
