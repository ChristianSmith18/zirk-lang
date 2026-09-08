Milestones are sequenced: nothing in a later group starts until the prior
group's milestone runs. Group 2 (the context-switch shim) is the hard gate — if
it cannot be made to work on every target, the whole approach is reconsidered
before anything is built on top.

## 1. Decisions locked before coding

- [x] 1.1 Write `docs/decisions/ADR-017-modelo-de-suspension.md`: stackful coroutines, single-threaded cooperative executor, no function coloring, no `async fn` (design D1); per-task shadow stack and generalized root enumeration, unchanged object header (design D2); alternatives (stackless state machine, segmented stacks, 1:1 threads) and why rejected
- [x] 1.2 Add an addendum to `docs/decisions/ADR-004-portabilidad.md` for the per-triple context-switch shim (macOS aarch64, Linux x86_64, Linux aarch64, Windows x86_64)
- [x] 1.3 Cross-link ADR-017 from `docs/decisions/ADR-003-memoria.md` and `docs/decisions/README.md` (also added the missing ADR-016 row)
- [x] 1.4 Lock the default task stack size — fixed 128 KiB, not user-configurable in this change; recorded in `design.md` "Resolved" section
- [x] 1.5 Lock `StackOverflowError` — ships in this change as a catchable `RuntimeError` from a cheap frame-prologue check; added to the `zirk-errors` delta and task 9.6
- [x] 1.6 Context-switch implementation: **`corosensei` 0.3**, target-gated so it only compiles on the four supported triples; thread-backed fallback for any other host. Recorded in `design.md` and `context.rs`. (Chosen up front rather than after 2.2 because `corosensei` *is* the switch — 2.2 now verifies it rather than choosing between candidates.)

## 2. Runtime: context-switch shim (`zirk-runtime`)

- [x] 2.1 Added `crates/zirk-runtime/src/context.rs` with the seam: `TaskContext` (owns the stack), `TaskContext::new(stack_bytes, body)` / `resume() -> Run` / `is_finished()`, and `Suspender::suspend()` handed to the body. Shape follows `corosensei`'s coroutine model rather than a raw `switch(from,to)` — the executor drives via `resume`, the task yields via `suspend`. `DEFAULT_TASK_STACK_BYTES = 128 KiB`.
- [x] 2.2 Native backend = `corosensei` 0.3, target-gated to the four supported triples via `build.rs` (`task_context_native` cfg) and a matching `[target.'cfg(...)']` dependency. Host-only thread-backed fallback (rendezvous channels, one-runner-at-a-time, drop-unwinds a suspended task) for any other host, same API.
- [x] 2.3 `corosensei` primes the fresh stack itself (`Coroutine::with_stack`); the fallback does the equivalent by not running the body until the first `resume()`. No separate `make_context` needed with this backend.
- [x] 2.4 `executor_and_task_ping_pong_preserves_task_local_state`: 1000 resume/suspend round trips, task-stack local + shared atomic checked on both sides across every switch. Plus: run-to-completion, independent interleaving of two contexts, and drop-unwinds-a-suspended-context.
- [x] 2.5 CI already runs `cargo test --workspace` on all four matrix triples (`linux-x86_64`, `linux-aarch64`, `windows-x86_64`, `macos-aarch64`) with `--test-threads=1`; the `context::tests` run there automatically. `task_context_native` is set on all four, so the thread fallback is never exercised on CI.
- [x] 2.6 Documented in `docs/ZIRK_RUNTIME_SPEC.md` §3 ("Implementation status (roadmap Phase 5, steps 1–3)").

## 3. Runtime: executor and task control block (`zirk-runtime`)

- [ ] 3.1 Add `crates/zirk-runtime/src/task.rs` with the TCB layout from design D4 (`state`, `stack`, `context`, `shadow_stack_head`, `result`, `result_consumed`, `scope`, sibling/child links, `cancel_requested`/`cancel_reason`, `shield_depth`, `waiter`, `cleanup_state`)
- [ ] 3.2 Add `crates/zirk-runtime/src/executor.rs`: single-threaded loop, FIFO ready `VecDeque<TcbPtr>`, `CURRENT_TASK` global set on every resume
- [ ] 3.3 Implement task allocation (`zirk_rt_task_spawn`): allocate stack + TCB, `make_context`, link into current scope, enqueue ready
- [ ] 3.4 Implement `zirk_rt_task_suspend` / resume: save context to `CURRENT_TASK`, switch to executor; executor re-enqueues and switches back on resume
- [ ] 3.5 Implement `zirk_rt_task_await`: register caller as `waiter`, suspend; on target completion produce `result` or re-raise `Throwable`; set `result_consumed`; `fatalError` on a second consume (runtime backstop)
- [ ] 3.6 Implement the unresolvable-wait detector: empty ready queue + no armed timer + a still-suspended task ⇒ abort with a diagnostic
- [ ] 3.7 Implement TCB/stack reclamation: free only after `cleanup_state` is done and no root chain references the TCB
- [ ] 3.8 Add `crates/zirk-runtime/src/timer.rs`: monotonic min-heap of deadlines (reuse the clock behind `Duration`/temporal `now_*`); `arm(deadline) -> TimerId`, `disarm(TimerId)`, `poll_expired(now)`; reject negative durations
- [ ] 3.9 Wire the executor lifecycle into `crates/zirk-runtime/src/lib.rs`: `zirk_rt_main` runs `main`'s body as root task 0, loops until task 0 + descendants finish, returns the exit code; uncaught escape ⇒ nonzero exit
- [ ] 3.10 Register `executor`, `task`, `timer`, `context` modules in `lib.rs`
- [ ] 3.11 Unit tests: two tasks interleave at `await`; ready-queue FIFO fairness; idle executor wakes at nearest deadline; unresolvable-wait abort

## 4. Runtime: garbage-collection integration (`zirk-runtime/src/collector.rs`)

- [ ] 4.1 Move the shadow-stack head from the process-global in `collector.rs` to `TCB.shadow_stack_head`; `zirk_rt_push_frame`/`zirk_rt_pop_frame` operate on `CURRENT_TASK`
- [ ] 4.2 Expose a "live TCB" iterator from the executor for the collector (ready, running, suspended, cleaning)
- [ ] 4.3 Change `collect()` root enumeration to walk every live task's chain, keeping `mark_clone_roots` as the extra set
- [ ] 4.4 Keep the object header unchanged (assert three words per ADR-012 in a test)
- [ ] 4.5 Tests: reference in a named local across `await` survives forced collection; reference only in a compiler-spilled temporary across `await` survives; finished+consumed task stops rooting its result and its stack is reclaimed; a cycle owned only by a suspended task is still reachable
- [ ] 4.6 Update `docs/decisions/ADR-003-memoria.md` and `docs/ZIRK_RUNTIME_SPEC.md` with the per-task root chain

## 5. Lexer + AST + parser (`zirk-lexer`, `zirk-ast`, `zirk-parser`)

- [ ] 5.1 `zirk-lexer`: make `select`, `scope`, `shield`, `timeout`, `after`, `cancelled` contextual keywords (identifiers elsewhere); `task`/`await` already lex
- [ ] 5.2 `zirk-ast`: add `TaskExpr { body: CallOrBlock }`, `TaskScopeExpr { block }`, `AwaitExpr { operand, timeout: Option<Expr> }`, `SelectExpr { arms: Vec<SelectArm>, default: Option<Block> }`, `SelectArm { guard, pattern, body }`, `CancellationShieldStmt { block }`, with spans on every node
- [ ] 5.3 `zirk-parser`: parse `task expr`, `task { block }`, `task scope { block }` as expressions
- [ ] 5.4 `zirk-parser`: parse `await expr` and `await expr timeout duration`
- [ ] 5.5 `zirk-parser`: parse `select { pattern = await op => body, after d => body, cancelled => body, default => body }`; reject a second `default`
- [ ] 5.6 `zirk-parser`: parse `cancellation shield { block }`
- [ ] 5.7 `zirk-parser`: reject `break`/`continue` that would cross a `task scope` boundary; keep `parallel`/`thread`/`task.blocking` on the "outside the subset" diagnostic
- [ ] 5.8 Lexer + parser tests: every form above, the `timeout`/`select`/`after` identifier fallback, the duplicate-`default` and illegal-jump diagnostics

## 6. Type system: async core typing (`zirk-sema`)

- [ ] 6.1 Register known types `Task<T>`, `Channel<T>`, `CancellationReason`; remove them and `TaskSettlement`/`Task`/`Channel` from the `pending_type` table
- [ ] 6.2 Register the compiler-known generic enum `TaskSettlement<T> { Fulfilled(T), Rejected(Throwable), Cancelled(CancelledError) }` (reuse the `Result`/`Either` generic-enum registration path)
- [ ] 6.3 Type `task expr` as `Task<T>` and `task scope { block }` as `Task<T>` from the block result
- [ ] 6.4 Type `await handle` as exactly `T`; type `await expr timeout d` as `T` and require `d: Duration`; no implicit `Result`/nullable/exception wrapping
- [ ] 6.5 Enforce single-consume: track the handle binding linearly, reject a second `await`, point at the first; must-use diagnostic for an ignored `Task<T>`; `_ = handle` discharges without detaching
- [ ] 6.6 Reject observing one `Task<T>` result from more than one place; name `watch`/`broadcast`/channel/shared-immutable
- [ ] 6.7 Implement compiler-known `Task.all` / `Task.first` / `Task.settled` static methods with the result types from the `zirk-type-system` delta
- [ ] 6.8 Type `select` arms: guard operands typed once in source order; branch value types unified; `after` operand `Duration`; `cancelled`/`default` valueless guards
- [ ] 6.9 Checker tests: await unwrap, timeout operand error, second-await rejection, multi-observer rejection, aggregation result types, select arm unification

## 7. Type system: Transfer / Share analysis — roadmap step 3 (`zirk-sema`)

- [ ] 7.1 Add the derived `Transfer` / `Share` property computation over every type (value/projection ⇒ `Transfer` by copy; complete `inmut::strict` ref ⇒ `Transfer` + `Share`; exclusive mutable complete ref ⇒ `Transfer` by move; `clone()` result ⇒ `Transfer`; handles/endpoints/pointers/dependent views ⇒ contract-gated)
- [ ] 7.2 Reuse the Phase 4d capture analysis and Phase 4e `transfer(r)` move/liveness machinery for the exclusive-move check (sender cannot use a transferred ref until it returns via a structured result or a channel)
- [ ] 7.3 Enforce the properties at `task` creation + captures, `task scope` results, channel `send`/`receive`, and `select` branch values
- [ ] 7.4 Reject an overlapping mutable alias crossing a boundary with a diagnostic naming transfer / strict-immutable share / `clone()` / channel
- [ ] 7.5 Apply the ordinary projection rule to a projected capture (`users[0]` ⇒ independent value)
- [ ] 7.6 Checker tests: value copy, strict-immutable share, exclusive move + use-after-transfer rejection, ambiguous mutable capture rejection, projected capture independence, logical-race rejection across a suspension

## 8. IR and lowering (`zirk-ir`, `zirk-ir/src/lower.rs`, `zirk-ir/src/verify.rs`)

- [ ] 8.1 Add IR instructions: `TaskStart`, `Suspend`/`ResumePoint`, `ScopeEnter`/`ScopeExit`, `ShieldEnter`/`ShieldExit`, `SelectRegister`/`SelectWait`/`SelectPick`, `TimerArm`/`TimerDisarm`, plus channel-op runtime-call kinds
- [ ] 8.2 Lower `task expr` / `task { }` to `TaskStart` registered with the current scope; lower `await` to `Suspend` + `ResumePoint` yielding the element value or re-raising
- [ ] 8.3 Lower `task scope { block }` to `ScopeEnter` / body / `ScopeExit`, installing `ScopeExit` as a cleanup handler on every exit edge (normal, `return`, unwind) via the Phase 4b/4c/4e cleanup-edge mechanism
- [ ] 8.4 Emit no `ScopeEnter`/`ScopeExit` for a function body that creates no child task
- [ ] 8.5 Lower `cancellation shield { }` to `ShieldEnter` / body / `ShieldExit` on every exit edge, with a pending-cancellation delivery check right after `ShieldExit`
- [ ] 8.6 Lower `await expr timeout d` to `TimerArm` + await + (timer-first ⇒ cancel operation, await cleanup, raise `TimeoutError`) + `TimerDisarm`
- [ ] 8.7 Lower every safe point to include a cancellation check that raises `CancelledError` when pending and `shield_depth == 0`
- [ ] 8.8 Lower `select` per design D10 (evaluate operands once in order; register; default-or-suspend; fair pick; deregister losers without cancelling; bind + run)
- [ ] 8.9 Lower `channel.send`/`receive` to possibly-suspending runtime calls; `try_send`/`try_receive` to non-suspending calls returning a typed outcome; apply the `Transfer` rule to crossing values
- [ ] 8.10 Extend `verify.rs` for the new instructions (type checks, cleanup-edge coverage, resume-point dominance)
- [ ] 8.11 IR lowering tests: task/await shape, scope cleanup edge on every path, no-scope for non-async body, shield defer, timeout branch, select shape, channel op shape

## 9. Native codegen (`zirk-codegen-llvm/src/emit.rs`, `src/runtime.rs`)

- [ ] 9.1 Emit each `Suspend` as a call to `zirk_rt_task_suspend` plus a resume label the executor returns to
- [ ] 9.2 Keep the ordinary function calling convention for every function regardless of internal suspension points (assert with a golden test comparing an awaiting and a non-awaiting function)
- [ ] 9.3 Emit `zirk_rt_push_frame`/`pop_frame` against `CURRENT_TASK` (design D2)
- [ ] 9.4 Emit program startup that enters the executor and runs `main` as root task 0
- [ ] 9.5 Declare the new runtime symbols in `runtime.rs`
- [ ] 9.6 Emit a cheap frame-prologue stack-limit check (compare against the running task's stack limit in the TCB) that raises `StackOverflowError` (locked in task 1.5)
- [ ] 9.7 Codegen golden tests: suspension point, resume label, per-task frame registration, executor entry

## 10. Runtime: scopes, failure, cancellation, shields, timeout

- [ ] 10.1 Implement the structured scope object (child list, state) and `ScopeEnter`/`ScopeExit` runtime hooks
- [ ] 10.2 Implement scope-exit join: normal exit awaits all children
- [ ] 10.3 Implement sibling-failure propagation (design D6): first unhandled child exception ⇒ primary; cancel active siblings; await cleanup; append secondary/cleanup failures to `suppressed()`; propagate
- [ ] 10.4 Implement `zirk_rt_task_cancel(handle, reason?)`: idempotent flag, default `CancellationReason.Cancelled`, recursive to descendants
- [ ] 10.5 Implement safe-point cancellation delivery: raise `CancelledError` unless `shield_depth > 0`; `CancelledError` flows through `try`/`catch`/`finally`
- [ ] 10.6 Implement `ShieldEnter`/`ShieldExit` depth accounting and deferred delivery immediately after the shield
- [ ] 10.7 Implement `await ... timeout`: timer-first path cancels the operation, awaits cleanup, raises `TimeoutError`; operation never continues in the background
- [ ] 10.8 Runtime tests: scope waits for a running child; one child throws ⇒ siblings cancelled+cleaned before propagation; returned `Result.Error` does not cancel siblings; cancel during shielded cleanup delivered after; timeout cancels+cleans+raises

## 11. Runtime: aggregation and `TaskSettlement`

- [ ] 11.1 Implement `zirk_rt_task_all`: input order preserved; first unhandled failure cancels the rest, awaits cleanup, propagates with others suppressed
- [ ] 11.2 Implement `zirk_rt_task_first`: first completion (success or unhandled failure) wins; cancel + clean the rest
- [ ] 11.3 Implement `zirk_rt_task_settled`: every task finishes; input order preserved; no sibling cancellation on a rejection; `Task<Result<U,E>>` returning `Error(e)` ⇒ `Fulfilled(Error(e))`; only an unhandled throwable ⇒ `Rejected`
- [ ] 11.4 Runtime tests: all-order + cancel-on-first-failure; first-wins + cancel-rest; settled with fulfilled/rejected/cancelled in input order

## 12. Runtime: `select`

- [ ] 12.1 Implement guard registration/deregistration for channel send/receive, task completion, timer, and cancellation guards
- [ ] 12.2 Implement fair ready-guard pick (rotating offset); non-suspending `default`
- [ ] 12.3 Leave losing operations alive and unregistered (not cancelled); channel closure is a ready receive outcome
- [ ] 12.4 Runtime tests: message-before-timer; multi-ready fairness/no-starvation; `default` runs only when nothing ready; losing task keeps running; closed channel selected

## 13. Runtime: `Channel<T>` core (`zirk-runtime/src/channel.rs`)

- [ ] 13.1 Implement the bounded channel: ring buffer of capacity `n`, suspended-sender FIFO, suspended-receiver FIFO, backpressure on full
- [ ] 13.2 Implement the zero-capacity rendezvous channel (send completes only when paired with a receive)
- [ ] 13.3 Implement `Channel.unbounded(limit:)` with a mandatory defense limit: backpressure or documented typed failure at the limit
- [ ] 13.4 Implement suspendible `send`/`receive`, non-suspending `try_send`/`try_receive` with a typed outcome (value / full-or-empty / closed)
- [ ] 13.5 Implement idempotent `close()`: wake all suspended senders/receivers; queued values drain before closure is observed
- [ ] 13.6 Implement `is_closed`, `capacity`, `length`
- [ ] 13.7 Register a custom GC trace hook for the channel object (walk queued reference values as roots); register `channel` in `lib.rs`
- [ ] 13.8 Runtime tests: backpressure; rendezvous pairing; drain-before-close; try-op typed outcomes; unbounded limit; queued reference survives forced collection

## 14. Runtime: broadcast / watch / one-shot channel families

- [ ] 14.1 Implement `broadcast`: each live subscriber receives every value posted after it subscribed; same closure/drain rules
- [ ] 14.2 Implement `watch`: latest value only; a new subscriber immediately sees the current value
- [ ] 14.3 Implement one-shot: exactly one value then auto-closed
- [ ] 14.4 Apply the same `Transfer`/cancellation rules and GC trace as the standard channel
- [ ] 14.5 Runtime tests: broadcast fan-out + late subscriber; watch latest-value; one-shot single delivery

## 15. Errors wiring (`zirk-sema`, `zirk-runtime`, `zirk-diagnostics`)

- [ ] 15.1 Register `CancelledError`, `TimeoutError`, and `StackOverflowError` as compiler-known concrete `RuntimeError` subclasses, catchable without `throws`
- [ ] 15.2 Runtime throwable construction for all three, with `message`/`code`/`cause`/`stack` behavior and deep immutability (Phase 4b machinery)
- [ ] 15.3 Ensure task rejection vs `Result.Error` separation is honored end-to-end (no implicit conversion either way)
- [ ] 15.4 Ensure suppressed-failure aggregation during structured cancellation preserves the primary's identity/origin/cause/stack
- [ ] 15.5 Error tests: cancellation caught without declaration; timeout caught without declaration; shielded cancellation delivered after; uncaught cancellation ends the task; sibling cleanup throw lands in `suppressed()`

## 16. Feature phasing and gating

- [ ] 16.1 Update the phase-diagnostic tables so `task`/`await`/`select`/`cancellation shield`/`Task<T>`/`Channel<T>`/`TaskSettlement<T>` are implemented; `parallel`/`thread`/`task.blocking`/`Mutex`/`RwLock`/`Semaphore`/`Barrier`/`Once`/`Atomic` still name their Phase 5 sub-step
- [ ] 16.2 Update `docs/init/ZIRK_ROADMAP.md` Phase 5: mark steps 1–3 delivered (single-threaded executor), steps 4–6 pending
- [ ] 16.3 Update `docs/init/ZIRK_FEATURE_STATUS.md` Phase 5 rows: `task`/`await` and `Channel<T>` to implemented across lexer→runtime→CLI; `parallel`/`thread`/`Atomic<T>` unchanged
- [ ] 16.4 Gating tests: `parallel`/`thread`/`Atomic` still diagnose with the right step; `task`/`await`/`Channel<T>` no longer diagnose

## 17. End-to-end CLI fixtures (`crates/zirk-cli/tests`)

- [ ] 17.1 `task` + `await` over a call; `task { block }`; `task scope` combining two children
- [ ] 17.2 Sibling failure: one child throws, siblings cancelled and cleaned, primary propagates with suppressed
- [ ] 17.3 `Result.Error` returned from a task does not cancel siblings
- [ ] 17.4 Cooperative cancellation with cleanup; `cancellation shield` deferring delivery
- [ ] 17.5 `await operation timeout d` success path and expiry path
- [ ] 17.6 `Task.all` order + cancel-on-failure; `Task.first`; `Task.settled` with mixed settlements
- [ ] 17.7 `select`: message before timer; `default`; `cancelled` guard; closed-channel guard
- [ ] 17.8 `Channel<T>`: bounded backpressure, rendezvous, drain-before-close, `try_*` outcomes, unbounded limit
- [ ] 17.9 broadcast / watch / one-shot fixtures
- [ ] 17.10 Transfer/Share rejections: overlapping mutable capture, use-after-transfer, logical race across a suspension — each with the expected diagnostic
- [ ] 17.11 GC-under-async fixture: allocation pressure while tasks are suspended holding references

## 18. Documentation and website sync

- [ ] 18.1 Update `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` implementation-checklist status for the delivered items
- [ ] 18.2 Update `docs/ZIRK_RUNTIME_SPEC.md` (executor, scheduler, timer, channels, context switch, per-task roots)
- [ ] 18.3 Update `docs/ZIRK_LANGUAGE_SPEC.md` and `docs/CORE_LANGUAGE_SEMANTICS.md` to mark `task`/`await`/`select` as implemented subset
- [ ] 18.4 Update the handbook `02-handbook/18-concurrency/` chapter and add `11-reference` pages for `Task<T>`, `Channel<T>`, `TaskSettlement<T>`
- [ ] 18.5 Add worked `.zrk` examples for the concurrency chapter
- [ ] 18.6 After the crates land and are committed, run `./scripts/sync-website-content.sh`; review the site-owned status catalog for the Phase 5 rows and the reduced concurrency limitations; run with `--audit-date YYYY-MM-DD`
- [ ] 18.7 Verify `../zirk-lang-site` no longer lists `task`/`await`/`Channel<T>` as unimplemented and that Phase 5 status is accurate

## 19. Closeout

- [ ] 19.1 Full workspace build + test on every supported target (`LLVM_SYS_201_PREFIX` set)
- [ ] 19.2 Run the whole CLI fixture suite; confirm no regression in Phase 1–4 fixtures
- [ ] 19.3 `openspec validate fase-5-async-core --strict`
- [ ] 19.4 Confirm the "explicitly out of scope" list (steps 4–6, multi-threaded GC, async I/O reactor) is untouched and still diagnoses correctly
