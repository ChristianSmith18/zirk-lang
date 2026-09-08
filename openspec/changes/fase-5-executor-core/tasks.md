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

- [x] 6.1 `Executor { registry: TaskRegistry, ready: VecDeque<TaskId>, timers: TimerService, root: Option<TaskId> }`
- [x] 6.2 Thread-locals `EXEC: Cell<*mut Executor>` (raw pointer, set only for `run()`) and `CURRENT_TASK: Cell<Option<TaskId>>`; `with_exec` creates a short-lived `&mut` that never spans a `resume` — the reentrancy contract in the module doc
- [x] 6.3 `spawn(body) -> TaskId` (free fn, uses `EXEC`): insert a `TaskContext` + slot, `ready.push_back` — not resumed until the loop picks it up
- [x] 6.4 `suspend_current(reason)` / `yield_now()` / `sleep(delay_nanos)`: record the `WaitReason` on the current slot, then `context::suspend_current()`; `sleep` arms a timer keyed by the task id first
- [x] 6.5 `await_task(id) -> usize` per design D5: dead-id ⇒ `fatal`, terminal + unconsumed ⇒ fast path, second consume / second waiter ⇒ `fatal`, else register waiter + suspend; a `Panicked` outcome re-unwinds into the awaiter
- [x] 6.6 The loop: `wake_expired_timers` → pop ready (else all-terminal ⇒ break, or sleep to the nearest deadline); `run_one_turn` takes the context out, resumes under `catch_unwind`, puts it back; on `Suspended` re-enqueue only `Yielded`; on `Finished`/panic → `complete` (outcome, wake waiter, reclaim)
- [x] 6.7 Unresolvable wait: empty ready queue + not all terminal + no armed timer ⇒ `panic!` with a diagnostic (unwinds out of the executor ⇒ nonzero exit, not a hang)
- [x] 6.8 `run_with_root(body) -> TaskOutcome`: spawn the root, run the loop until every live task is terminal, return the root's outcome
- [x] 6.9 `reclaim()` (design D7): free a slot when terminal + `result_consumed` + `cleanup_state == Done` + not root + not current. Never-awaited tasks linger until the executor drops (detach flag arrives with the language surface)
- [x] 6.10 Tests (14 total): root returns a value; two children round-robin at `yield_now` ("ababab"); `await` produces the child value; already-finished fast path; FIFO across 5 ready tasks; the program waits for a background child after the root returns; a child panic surfaces when awaited; `sleep` wakes via the timer while a sibling runs; an idle executor with only a sleeping task finishes; an unresolvable wait aborts; consumed children are reclaimed

## 7. `collector.rs`: per-task root chains

- [x] 7.1 `FRAMES` split: `BOOTSTRAP_FRAMES` (the pre-task-0 / no-task-running chain) + `ACTIVE_FRAMES: Cell<*mut Vec<Frame>>` (the running task's chain, pointed at a `Vec` inside its boxed control block by the executor)
- [x] 7.2 `zirk_rt_push_frame` / `zirk_rt_pop_frame` go through `with_active_frames` — the active task's chain, or the bootstrap chain when `ACTIVE_FRAMES` is null
- [x] 7.3 `mark()` walks the bootstrap chain, then calls the executor-registered `TaskRootWalker` (visits every live task's chain), then `mark_clone_roots` as before. `set_active_frames` / `set_task_root_walker(Option<..>)` are the executor's hooks; a `Restore` guard clears both on every run exit including a panic
- [x] 7.4 Object header untouched (three words, `ADR-012`) — no change to `HEADER_BYTES` / `NEXT_WORD` / `SIZE_WORD`; the existing header/layout tests still pass
- [x] 7.5 `reset_state` / `reset_state_for_tests` now clear `BOOTSTRAP_FRAMES` and null `ACTIVE_FRAMES`; every pre-existing collector test drives the bootstrap chain unchanged
- [x] 7.6 `a_suspended_tasks_roots_survive_a_collection_from_another_task`: root task roots an object through a pushed frame, `yield_now`s; a sibling calls `collect()`; the object survives (accounted in `LIVE_BYTES`, header intact) and is reclaimed only after the frame pops. (The compiler-spilled-temporary variant is a codegen behavior — `fase-5-structured-tasks` — the D4 spill rule; the finished-task-stops-rooting case is covered by executor test `an_awaited_child_slot_is_reclaimed_after_consumption`.)
- [x] 7.7 `cargo test --workspace` — no regression (see closeout)

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
