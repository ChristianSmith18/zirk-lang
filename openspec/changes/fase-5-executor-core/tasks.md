Groups 1–2 were delivered on this branch before the change was split out
(`ADR-017`, `crates/zirk-runtime/src/context.rs`). They are listed done for
traceability; work starts at group 3.

## 1. Suspension model (done)

- [x] 1.1 `docs/decisions/ADR-017-modelo-de-suspension.md` — stackful coroutines, single-threaded cooperative executor, no function coloring; per-task shadow stack; unchanged object header
- [x] 1.2 `ADR-004` addendum for the per-triple context-switch shim
- [x] 1.3 Cross-links from `ADR-003` and the decisions index
- [x] 1.4 Task stack fixed at 128 KiB, not configurable
- [x] 1.5 `StackOverflowError` ships as a catchable `RuntimeError` (spec recorded in `zirk-errors`)
- [x] 1.6 Context-switch backend: `corosensei` 0.3, target-gated; thread-backed fallback for other hosts

## 2. Context seam (done)

- [x] 2.1 `crates/zirk-runtime/src/context.rs`: `TaskContext::{new,resume,is_finished}`, `Suspender::suspend`, `Run`, `DEFAULT_TASK_STACK_BYTES`
- [x] 2.2 `build.rs` `task_context_native` cfg + `[target.'cfg(...)']` corosensei dependency
- [x] 2.3–2.4 1000-round executor↔task ping-pong test, run-to-completion, interleaving, drop-unwinds-a-suspended-context
- [x] 2.5 CI runs `context::tests` on all four matrix triples via `cargo test --workspace`
- [x] 2.6 `docs/ZIRK_RUNTIME_SPEC.md` §3 executor/context note

## 3. `context.rs`: `suspend_current` and a result channel

- [x] 3.1 Thread-local `ARMED: Cell<*const Yielder>` (native); set at body entry (`ARMED.replace`), re-armed at the end of every `Suspender::suspend` and `suspend_current`
- [x] 3.2 `pub fn suspend_current()` — asserts `ARMED` non-null, suspends through it, re-arms; re-exported from `lib.rs`
- [x] 3.3 `TaskContext` body is `FnOnce(&Suspender) -> usize`; `Run::Finished(usize)` carries it; Group 2 tests updated
- [x] 3.4 Fallback backend mirrors: per-thread `ARMED: Cell<*const Suspender>` set at body entry (each fallback task is its own thread, so the thread-local is already per-task), `Step::Finished(usize)`
- [x] 3.5 Tests: `suspend_current_round_trips_like_the_suspender`, `three_tasks_suspending_via_suspend_current_interleave` (14 == 1·1+2·2+3·3), `a_body_result_reaches_resume` (0xC0FFEE). 7 context tests pass, clippy/fmt clean.

## 4. `task.rs`: task control block and registry

- [x] 4.1 `TaskId { index: u32, generation: u32 }`, `to_bits`/`from_bits` (`generation << 32 | index`)
- [x] 4.2 `TaskState` (+ `is_terminal`), `WaitReason` (`None`/`Yielded`/`AwaitingTask`/`Timer`), `TaskOutcome` (`Value(usize)` / `Panicked(Box<dyn Any + Send>)`), `CancelReason` (`#[default] Cancelled`, `TimedOut`), `CleanupState`
- [x] 4.3 `TaskControlBlock` per design D2: `state`, `context`, `outcome`, `result_consumed`, `waiter`, `wait`, `cancel_requested`/`cancel_reason`, `shield_depth`, `roots: Vec<collector::Frame>` (group 7 wires push/pop/walk), `cleanup_state` (trivially `Done` here)
- [x] 4.4 `TaskRegistry`: generational slab (`Vec<Slot>` + free list + `live` count); `insert -> TaskId`, `get`/`get_mut` with generation check, `remove` (bumps generation, drops `TaskContext` ⇒ frees stack), `live_ids()`
- [x] 4.5 Tests: `to_bits` round-trip; insert/get/remove; a reused slot rejects the old id; `live_ids` skips freed slots. (`#[allow(dead_code)] mod task` until the executor consumes it in group 6.)

## 5. `timer.rs`: monotonic timer service

- [x] 5.1 `std::time::Instant` (monotonic — deliberately not the `SystemTime` clock the temporal `now_*` use, so a wall-clock jump can't disturb scheduling); `arm` rejects a negative delay with `NegativeDuration` before scheduling
- [x] 5.2 `TimerService`: `BinaryHeap<Entry>` ordered earliest-first; `arm(delay_nanos, payload) -> Result<TimerId, NegativeDuration>`, `disarm(TimerId)` (lazy — drops at heap top), `poll_expired(now) -> Vec<(TimerId, u64 payload)>`, `peek_deadline()`, `is_empty()`
- [x] 5.3 `disarm` of an unknown / already-fired / already-cancelled id is a no-op
- [x] 5.4 Tests: deadline-order firing; disarm before fire; disarm of unknown/fired id; `peek_deadline` bounds the idle sleep; negative delay rejected. (`#[allow(dead_code)] mod timer` until group 6.)

## 6. `executor.rs`: the loop

- [ ] 6.1 `Executor` struct: `TaskRegistry`, `VecDeque<TaskId>` ready queue, `TimerHeap`, root task id
- [ ] 6.2 Thread-local `CURRENT_TASK: Cell<Option<TaskId>>`; set on every resume, cleared between turns
- [ ] 6.3 `spawn(body) -> TaskId`: allocate `TaskContext` + slot, enqueue Ready (do not resume yet — design D6)
- [ ] 6.4 `suspend_current(reason: WaitReason)`: record the reason in `CURRENT_TASK`'s slot, then `context::suspend_current()`
- [ ] 6.5 `await_task(id) -> TaskOutcome` per design D5 (generation check, already-done fast path, second-consume `fatalError`, else register waiter + suspend)
- [ ] 6.6 The loop per design D6: poll timers → wake waiters; pop ready or sleep/abort; `resume`; on `Suspended` leave parked (re-enqueue only `Yielded`); on `Finished` set outcome, wake waiter, run reclamation
- [ ] 6.7 Unresolvable-wait detector: empty ready queue + empty timer heap + a still-`Suspended` task ⇒ abort with a diagnostic naming the condition
- [ ] 6.8 `run_until_root_done()`: drive the loop until the root task and all live tasks are terminal
- [ ] 6.9 Reclamation (design D7): free a slot when terminal + result consumed (or no waiter) + `cleanup_state == Done` + not current
- [ ] 6.10 Tests: two tasks interleave at `suspend_current`; FIFO fairness across 5 ready tasks; `await` produces the awaited `usize` exactly once and a second `await` aborts; idle executor wakes at the nearest deadline; unresolvable-wait aborts instead of hanging; a spawned task runs before its parent's next suspension point

## 7. `collector.rs`: per-task root chains

- [ ] 7.1 Move the shadow-stack head from the process-global into `TaskSlot.shadow_stack`; add a bootstrap chain used before task 0 exists and folded into task 0 on spawn
- [ ] 7.2 `zirk_rt_push_frame` / `zirk_rt_pop_frame` resolve `CURRENT_TASK` (bootstrap chain when none)
- [ ] 7.3 `collect()` walks the bootstrap chain then every live task's chain via an executor-provided `for_each_live_task_roots`; keep `mark_clone_roots` as the separate extra set
- [ ] 7.4 Assert the object header is still three words (`ADR-012`)
- [ ] 7.5 Update existing collector tests that assumed the global head (drive an explicit task 0 / bootstrap chain)
- [ ] 7.6 New tests: reference in a named local across a `suspend_current` survives a forced collection; reference only in a compiler-spilled temporary across a suspend survives; a cycle owned only by a suspended task is reachable; a finished + consumed task stops rooting its result and its stack is reclaimed on the next collection
- [ ] 7.7 Run the full pre-existing allocation-pressure and cycle suite — no regression

## 8. `lib.rs`: executor lifecycle

- [ ] 8.1 `zirk_rt_main` (or the existing entry wrapper) creates task 0 from `main`'s body, runs `run_until_root_done()`, returns task 0's exit code
- [ ] 8.2 An exception escaping task 0 still yields a nonzero exit (unchanged behavior, routed through the executor)
- [ ] 8.3 Register `executor`, `task`, `timer` modules; wire `context::suspend_current` visibility
- [ ] 8.4 Test: a Rust-level `main` body that spawns a child and returns — the process (test harness) does not "finish" until the child is terminal

## 9. `extern "C"` surface (defined, Rust-tested, codegen-wired later)

- [ ] 9.1 `zirk_rt_task_spawn(body: extern "C" fn(*mut c_void) -> usize, arg: *mut c_void) -> u64`
- [ ] 9.2 `zirk_rt_task_await(id: u64) -> usize`, `zirk_rt_task_is_done(id: u64) -> bool`
- [ ] 9.3 Module docs: the packed-`u64` `TaskId` and `usize` result are provisional pending `fase-5-structured-tasks`
- [ ] 9.4 Tests exercising the `extern "C"` entry points directly (spawn a C-ABI body, await it, read the result)

## 10. Documentation and status

- [ ] 10.1 `docs/ZIRK_RUNTIME_SPEC.md` §3: fill in the timer service and the `main` lifecycle detail
- [ ] 10.2 `docs/init/ZIRK_FEATURE_STATUS.md`: add a Phase 5 row — executor / task control block / timer / per-task GC roots delivered; `task` / `await` / `select` still pending
- [ ] 10.3 `docs/init/ZIRK_ROADMAP.md` Phase 5: note the executor infrastructure is delivered ahead of the language surface
- [ ] 10.4 After the crate lands and is committed, run `./scripts/sync-website-content.sh` and pass `--audit-date YYYY-MM-DD` for the changed Phase 5 status

## 11. Closeout

- [ ] 11.1 `cargo test -p zirk-runtime` green; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] 11.2 `cargo test --workspace` — no regression in Phase 1–4 fixtures
- [ ] 11.3 `openspec validate fase-5-executor-core --strict`
- [ ] 11.4 Confirm no language-surface construct compiles (`task` / `await` still diagnose)
