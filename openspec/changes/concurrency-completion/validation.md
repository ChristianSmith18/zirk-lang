# concurrency-completion validation record

Validation date: 2026-10-13  
Baseline `HEAD`: `5250bbf154c1c8b1412f97e24411d160f9670e86`  
Rust: `rustc 1.94.1 (e408947bf 2026-03-25)`  
Checkout: dirty working tree; results below were run against the integrated working tree, not the baseline commit by itself. This change and unrelated user work remain uncommitted.

## Integrated checks

| Command | Result |
| `cargo test -p zirk-runtime --test gc_registered_participant_lifecycle -- --test-threads=1` | Passed: 1 host integration test. It exercised GC pressure in registered parallel workers and during executor-task park/cancel/join/shutdown; no non-host runtime execution or internal `InNative` transition is claimed. |
|---|---|
| `cargo test --workspace --quiet` (prior integrated checkpoint before the latest hardening) | Passed: 2,042 tests, 10 ignored, across 58 suites |
| `cargo test --workspace --quiet` (after descriptor-backed task results, observed forced-GC regression, terminal unwind root cleanup, 32-bit fail-closed gate, captured background root/drain stress, and mixed sync failure stress) | Passed: 2,051 tests, 10 ignored, across 58 suites |
| `cargo test --workspace --quiet` (after enabling protected cleanup and consuming a pending cleanup throwable when cancellation wins) | Passed: 2,058 tests, 10 ignored, across 58 suites |
| `cargo test --workspace --quiet` (after adding the bounded managed-task wait-any ABI and extending the GC result regression to enum payloads) | Passed: 2,060 tests, 10 ignored, across 58 suites |
| `cargo test --workspace --quiet` (after verified `JobWaitAny` IR, bounded LLVM ID-array emission, explicit interpreter/browser exclusion, invalid-count-before-pointer-read regression, and `Thread.run` transitive channel-effect fixture) | Passed: 2,068 tests, 10 ignored, across 58 suites |
| `cargo test --workspace --quiet` (after managed `.first()` integration, Wasm ABI consumer linking, first-capability gate updates, and browser/interpreter exclusion fixes) | Passed: 2,105 tests, 10 ignored, across 61 suites |
| `cargo test --workspace --quiet` (after exact-capacity channel/background-supervisor tests and `.settled()` cancellation-gap audit documentation) | Passed: 2,107 tests, 10 ignored, across 62 suites |
| `cargo test -p zirk-runtime --quiet` (after global `ManagedBlockingPool` worker-budget enforcement) | Passed: 559 tests, 7 ignored, across 16 suites |
| `cargo test --workspace --quiet` (after global managed-pool worker budget, concurrency ceiling regressions, and both additional runtime benchmarks) | Passed: 2,109 tests, 12 ignored, across 62 suites |
| `cargo test --workspace --quiet` (current integrated checkout, 2026-10-13) | Passed: 2,116 tests, 12 ignored, across 62 suites |
| `cargo test --workspace --quiet` (after adding generated-Zirk task-capacity boundary/reuse E2E and browser capability exclusion, 2026-10-13) | Passed: 2,131 tests, 12 ignored, across 63 suites |
| `cargo test -p zirk-runtime --release --test cancel_shutdown_gc_stress --test collector_concurrency_stress --test executor_cancel_model --test channel_cancel_wakeup_abi -- --test-threads=1` (current integrated checkout, 2026-10-13) | Passed: 5 tests across 4 suites |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after the global worker-budget change, additional benchmarks, and Linux ABI consumer script |
| `cargo fmt --all -- --check`; focused rustfmt check for both benchmark files; `bash -n scripts/validate-concurrency-abi-consumer-linux.sh`; `git diff --check`; `openspec validate concurrency-completion --strict` | All passed after the resource-ceiling, `.settled()` audit, benchmark, Linux-link-script, and final task/validation record edits |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after cleanup-failure promotion and typed first-cleanup lowering context integration |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed on the current checkout after task-capacity E2E and browser exclusion; format, diff, and strict OpenSpec validation also passed |
| `cargo fmt --all -- --check`; `git diff --check`; `openspec validate concurrency-completion --strict` | Passed after current `.first()`/ABI consumer integration |
| `cargo test -p zirk-interpreter --quiet` (after explicit `JobWaitAny` preflight rejection) | Passed: 53 tests; the interpreter has no task scheduler and fails closed for this instruction |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after `JobWaitAny` integration |
| `cargo fmt --all -- --check`; `git diff --check`; `openspec validate concurrency-completion --strict` | All passed after `JobWaitAny` integration; strict validation reports the change valid |
| `cargo test -p zirk-runtime --quiet` (current after captured background root and combined semaphore/Once stress regressions) | Passed: 532 tests, 5 ignored, across 12 suites |
| `cargo test -p zirk-codegen-llvm task_result_boxes_fail_closed_on_32_bit_targets -- --nocapture` | Passed: 1 test; an i386 target rejects task-result descriptor emission before writing an object |
| `cargo test -p zirk-runtime unwound_task_drops_shadow_frames_before_it_becomes_collectable --lib -- --nocapture` | Passed: 1 test; an unexpected native unwind leaves no terminal task stack roots registered |
| `cargo test -p zirk-runtime --test staticlib -- --test-threads=1` | Passed: 5 tests; includes public C ABI wait-any completion ordering, caller-cancellation sentinel, and rejection of out-of-range counts before inspecting the ID pointer |
| `cargo test -p zirk-runtime wait_any --lib -- --test-threads=1` | Passed: 2 scheduler tests for any-wait wakeup and cancellation waiter removal |
| `cargo test -p zirk-ir --test verification job_wait_any -- --test-threads=1` | Passed: 5 verifier cases for valid 2–8 Job operands, operand uses, invalid arity/types, and UInt64 result |
| `cargo test -p zirk-codegen-llvm --test emission job_wait_any_emits_a_bounded_id_array_and_runtime_call -- --exact --test-threads=1` | Passed: LLVM emission builds a fixed bounded ID array and calls the u64 wait-any ABI |
| `cargo check -p zirk-runtime --lib --target i686-unknown-linux-gnu` | Passed after adding the u64 wait-any ABI; confirms the ID/count/result contract compiles on a 32-bit target |
| `cargo check -p zirk-runtime --lib --target wasm32-unknown-unknown` | Passed after adding the u64 wait-any ABI |
| `cargo test -p zirk-ir --test verification job_wait_any -- --test-threads=1` | Passed: 5 verifier cases for valid 2–8 Job operands, operand uses, invalid arity/types, and UInt64 result |
| `cargo test -p zirk-codegen-llvm --test emission job_wait_any_emits_a_bounded_id_array_and_runtime_call -- --exact --test-threads=1` | Passed: LLVM emission builds a fixed bounded ID array and calls the u64 wait-any ABI |
| `cargo test -p zirk-cli --test end_to_end concurrent_join_round_trips_nested_managed_record_results -- --exact --test-threads=1` | Passed: 1 test; nested record and enum payload references survive an observed real GC collection while pending, and Int128 round-trips exactly |
| `cargo test -p zirk-runtime active_background_capture_root_survives_explicit_collection --lib -- --nocapture` | Passed: 1 test; an explicit collection preserves a captured background-task root and confirms it remains live |
| `cargo test -p zirk-runtime --test cancel_shutdown_gc_stress captured_background_root_survives_gc_pressure_and_root_completion_drains_cancellation -- --exact --nocapture` | Passed: 1 test; four repeated allocation-pressure/cancel/drain cycles complete and read the captured value |
| `cargo test -p zirk-runtime --test background_panic_lifecycle repeated_executor_runs_report_panics_and_finish_background_drain -- --exact --nocapture` | Passed: 1 test; 32 fresh executor runs report panics and acknowledge cooperative cleanup before root completion |
| `cargo test -p zirk-runtime bounded_sync_panic_cancel_and_wakeup_interactions_terminate --lib -- --test-threads=1` | Passed: 1 test; 16 rounds of semaphore and Once cancellation, panic, FIFO recovery, and completion |
| `cargo test -p zirk-runtime --quiet` (prior integrated checkpoint) | Passed: 528 tests, 5 ignored, across 12 suites |
| `cargo test -p zirk-runtime --lib -- --test-threads=1` | Passed after task/timer/waiter baseline assertions: 265 tests |
| `cargo test -p zirk-runtime --lib combinators::tests -- --test-threads=1` | Passed: 25 tests, including caller cancellation and timer cleanup regressions |
| `cargo test -p zirk-runtime --lib combinators::tests -- --test-threads=1` (current) | Passed: 26 tests, including cancellation after zero-deadline expiry and before timeout handling |
| `cargo test -p zirk-runtime --lib sync_mutex::tests -- --test-threads=1` | Passed: 8 tests for FIFO handoff, cancellation-safe active-owner behavior, executor progress, panic unlock, reentrancy, and thread affinity |
| `cargo test -p zirk-runtime --lib sync_rwlock::tests -- --test-threads=1` | Passed: 12 tests for reader cohorts, writer fairness, queued/handoff cancellation, executor progress, panic release, reentrancy, and thread affinity |
| `cargo test -p zirk-runtime --lib sync_mutex::tests::contended_mutex_handoff_preserves_fifo_across_many_rounds -- --exact` | Passed: 1 test; 32 rounds with 8 queued mutex waiters each preserved FIFO order |
| `cargo test -p zirk-runtime --lib sync_rwlock::tests::contended_rwlock_writers_preserve_fifo_across_many_rounds -- --exact` | Passed: 1 test; 24 rounds with 6 queued writers each preserved FIFO order |
| `cargo clippy -p zirk-runtime --lib -- -D warnings` | Passed after adding mutex/RwLock FIFO stress tests |
| `cargo test -p zirk-runtime --lib sync_primitives::tests -- --test-threads=1` | Passed: 17 tests for semaphore bounds/FIFO/RAII/close/cancellation, barrier generations/cancellation/broken wakeup, and Once publication/panic retry/cancellation/FIFO handoff. Two initially nondeterministic Once fixtures were made deterministic with atomic gates. |
| `cargo test -p zirk-runtime --lib sync_primitives::tests::bounded_sync_contention_cancellation_and_release_cycles_terminate -- --test-threads=1` | Passed after expanding the regression to 64 semaphore cancellation/release and barrier-cancellation rounds. |
| `cargo test -p zirk-runtime --lib sync_primitives::tests::bounded_sync_panic_cancel_and_wakeup_interactions_terminate -- --test-threads=1` | Passed after expanding the mixed semaphore/Once panic, cancellation, and FIFO-recovery regression to 32 rounds. |
| `cargo test -p zirk-runtime --test staticlib -- --test-threads=1` | Passed: 6 ABI tests, including bounded-count validation and isolated fatal-path checks for stale, duplicate, self, consumed, and already-claimed `JobWaitAny` IDs. |
| `cargo test -p zirk-runtime --lib wait_any -- --test-threads=1` | Passed: 2 scheduler tests for wait-any registration and completion behavior. |
| `cargo test -p zirk-runtime --test job_outcome_take -- --test-threads=1` | Passed: 3 focused tests covering wait-and-consume fulfillment, rejection and child cancellation; separate caller cancellation with the child result retained; GC rooting while waiting and after transfer; invalid handles/slots; duplicate consumption; native-panic fail-closed behavior; and null-Throwable rejection at the branch-failure ABI boundary. This validates runtime primitives only, not construction of a Zirk `Outcome<T>`. |
| `cargo test -p zirk-runtime --test scope_collect_all -- --test-threads=1` | Passed: 5 tests for early child failure with a slower sibling, multiple rejected children, child cancellation, caller cancellation cleanup, ordinary fail-fast preservation, and invalid/mismatched collect-scope handles. |
| `cargo test -p zirk-runtime --lib exceptions::tests -- --test-threads=1` | Passed: 3 tests, including suppressed-cause append without overwriting an existing chain, duplicate-edge suppression, cycle rejection, and GC edge pruning. |
| `cargo build -p zirk-runtime && cargo test -p zirk-runtime --test staticlib -- --test-threads=1` | Passed: rebuilt the static library and passed 6 tests, including unmangled symbol checks for `zirk_rt_job_outcome_take`, `zirk_rt_job_outcome_wait_take`, `zirk_rt_branch_fail`, and both collect-all scope ABI entries. |
| `cargo test -p zirk-runtime -- --test-threads=1` | Passed: 544 tests, 5 ignored, after the outcome wait-and-consume ABI, null-Throwable rejection, collect-all scopes, wait-any handle validation, and expanded synchronization stress. |
| `cargo clippy -p zirk-runtime --all-targets -- -D warnings` | Passed after the wait-and-consume ABI and null-Throwable rejection changes. |
| `cargo test -p zirk-ir` | Passed: 213 tests after adding `JobOutcomeWaitTake` verification for job operands, distinct pointer-shaped managed slots, and GC-root membership. |
| `cargo test -p zirk-codegen-llvm` | Passed: 65 tests after adding the exact outcome wait/take ABI declaration and slot-address emission regression. |
| Focused `JobOutcomeWaitTake` verifier and LLVM emission tests | Passed: 5 IR verifier cases and 1 LLVM emission case. |
| `cargo test --workspace --quiet` | Passed: 2,093 tests, 10 ignored, across 61 suites after direct managed `.settled()` lowering, nested-payload GC coverage, explicit browser exclusions for completion outcomes, fail-closed status handling, and atomic compare-exchange order matrix coverage. |
| `cargo test -p zirk-cli --test gc_generated_loop generic_outcome_with_nested_managed_payload_survives_gc -- --exact --nocapture` | Passed: the `Outcome<Payload>` value survived actual collections triggered by 20,000 allocating loop iterations with `ZIRK_GC_THRESHOLD=1`. |
| `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` | Passed: all valid fixtures, including generic `Outcome<Payload>` construction and matching. |
| `cargo test -p zirk-runtime --test atomic_compare_exchange_order_matrix -- --exact --test-threads=1` | Passed: enumerated all 25 compare-exchange success/failure ordering pairs for matching and mismatching values; invalid pairs leave the atomic unchanged. |
| `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` | Passed: `.settled()` fixtures fail closed with the explicit managed-completion capability diagnostic and are listed in the browser corpus exclusions. |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after removing redundant LLVM value conversions and a needless lowering temporary; focused IR lowering and LLVM outcome-ABI emission tests passed afterward. |
| `scripts/validate-wasm-staticlib-link.sh` | Passed: built the `wasm32-unknown-unknown` runtime static library, compiled an independent C consumer, and linked a valid Wasm module with undefined symbols disallowed. Link validation only; the module was not executed. |
| `cargo test -p zirk-runtime sync_primitives::tests::bounded_sync_contention_cancellation_and_release_cycles_terminate -- --exact --nocapture` | Passed: 1 test; 24 semaphore cancellation/release cycles and 24 cancelled Barrier generations, with progress, permit conservation, empty queues, and all participants terminating |
| `cargo test -p zirk-runtime --test background_panic_lifecycle repeated_executor_runs_report_panics_and_finish_background_drain -- --exact` | Passed: 1 test covering 32 fresh-executor background-panic/report/drain cycles |
| `cargo test -p zirk-runtime atomic::tests::c_abi_compare_exchange_reports_success_failure_and_rejects_orderings -- --exact --test-threads=1` | Passed: 1 test covering all 25 success/failure ordering pairs and unchanged outputs on invalid pairs |
| `cargo test -p zirk-ir atomic::tests --lib -- --test-threads=1` | Passed: 6 verifier tests, including unsupported pointer/aggregate/float/i128 values and malformed atomic ABI signatures |
| `cargo test -p zirk-runtime executor::tests::completed_job_managed_result_survives_collection_until_consumed -- --exact --nocapture` | Passed: 2 harness instances; a completed pointer result remains live through explicit collection until `Job.wait` consumes it |
| `cargo test -p zirk-runtime executor::tests::branch_failure_throwable_survives_collection_while_scope_waits -- --exact --nocapture` | Passed: 2 harness instances; a `BranchFailure` throwable remains live through explicit collection until awaited |
| `cargo test -p zirk-cli --test gc_generated_loop -- --exact generated_allocation_loop_preserves_a_reachable_object_graph_across_gc --nocapture` | Passed: 1 compiled-and-executed Zirk test with 20,000 loop allocations and `ZIRK_GC_THRESHOLD=1` |
| `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` | Passed: 59 tests, including nine `.within` deadline fixtures, explicit named Phase 5 diagnostics, and nested managed task-result GC round-trip |
| `cargo test -p zirk-cli --test end_to_end phase_five_completion_apis_report_named_pending_gates -- --exact --test-threads=1` | Passed: 1 test covering the still-gated `Concurrent.each` APIs and absence of generic undeclared-name fallbacks |
| `cargo test -p zirk-cli --test end_to_end pending_concurrency_types_report_the_phase_gate -- --exact --test-threads=1` | Passed: 1 test, including `Atomic<Pointer<Int32>>` rejection at the Phase 5 type gate |
| `cargo test -p zirk-runtime services::tests -- --test-threads=1` | Passed: 17 service lifecycle tests |
| `cargo test -p zirk-cli --test end_to_end the_invalid_corpus_is_rejected_with_diagnostics -- --exact --test-threads=1` | Passed: 1 test; includes `Concurrent.of` pending-shape and rejected-`Void`-result fixtures plus `Concurrent.protect` rejected-suspension fixture |
| `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` | Passed: 1 test; the valid corpus includes a runnable `Concurrent.protect` cleanup fixture |
| `cargo test -p zirk-sema concurrent_protect -- --test-threads=1` | Passed: 5 focused tests covering accepted zero-argument `Void` cleanup, rejected signatures/results, direct/transitive waits, channel send, blocking extern, unknown effects, spawning, and the still-gated `Thread.run` call |
| `cargo test -p zirk-ir --test lowering` | Passed: 162 tests; includes callback capture, early return, thrown exception, nested shields, and cancellation precedence with pending-exception consumption |
| `cargo test -p zirk-ir --test lowering cancellation_wins_over_a_cleanup_exception_and_consumes_it_before_cancel -- --exact --test-threads=1` | Passed: 1 test; confirms a cleanup throwable is consumed before terminal cancellation |
| `cargo test -p zirk-codegen-llvm --test emission concurrent_protect_emits_shield_calls_and_post_cleanup_cancellation_check -- --exact --test-threads=1` | Passed: 1 test; generated LLVM restores shield depth before its cancellation check |
| `cargo test -p zirk-sema -p zirk-ir -p zirk-codegen-llvm` | Passed: 900 tests across the selected compiler crates at integration time |
| Timer unit tests | Passed: 18 tests, including active registration baseline, cancellation, expiry, and tombstone compaction |
| `cargo test -p zirk-runtime --lib combinators::tests::caller_cancellation_wins_over_pending_within_timeout_and_joins_all_children -- --exact --test-threads=1` | Passed after fixing timer cancellation; verifies `first_within` and `settled_within` cleanup |
| `cargo test -p zirk-runtime --lib combinators::tests::repeated_timeouts_and_caller_cancellation_join_every_started_branch -- --exact --test-threads=1` | Passed after fixing timer cancellation; verifies waiter and timer registrations return to baseline across 16 timeouts and 8 caller cancellations |
| `cargo test -p zirk-sema` | Passed: 636 tests across 3 suites, including completion-terminal, timer-callback ownership, `Concurrent.protect`, and user-defined namespace shadowing regressions |
| `cargo test -p zirk-sema thread_run_rejects_transitive_suspension_in_a_named_function -- --exact --test-threads=1` | Passed: the named callback's timer, channel receive, and task-spawn effects are rejected with `E0484` while `Thread.run` remains gated with `E0424` |
| `cargo test -p zirk-sema completion_pipeline -- --test-threads=1` | Passed: 2 tests for explicit named gates, terminal arity, and Duration validation |
| `cargo test -p zirk-sema timer_callbacks_enforce_capture_and_result_ownership_boundaries -- --exact --test-threads=1` | Passed: 1 test for mutable callback capture and non-transferable callback result rejection |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after descriptor-backed task-result boxes, terminal unwind-frame cleanup, 32-bit fail-closed gate, task-outcome GC rooting, and namespace-shadowing fix |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after enabling `Concurrent.protect` and adding cancellation/cleanup throwable precedence handling |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed after adding the bounded task wait-any ABI and expanding managed result GC coverage to enums |
| `cargo clippy -p zirk-runtime --lib -- -D warnings` | Passed after simplifying a never-looping Barrier control-flow loop |
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed |
| `openspec validate concurrency-completion --strict` | Passed |

## Runtime target checks

For each Rust target below, both commands passed:

```text
cargo check -p zirk-runtime --tests --target <rust-target>
cargo build -p zirk-runtime --target <rust-target>
```

| LLVM codegen triple | Rust target used for runtime check/build |
|---|---|
| `i686-pc-windows-msvc` | `i686-pc-windows-msvc` |
| `x86_64-pc-windows-msvc` | `x86_64-pc-windows-msvc` |
| `aarch64-pc-windows-msvc` | `aarch64-pc-windows-msvc` |
| `i386-unknown-linux-gnu` | `i686-unknown-linux-gnu` |
| `x86_64-unknown-linux-gnu` | `x86_64-unknown-linux-gnu` |
| `armv7-unknown-linux-gnueabihf` | `armv7-unknown-linux-gnueabihf` |
| `aarch64-unknown-linux-gnu` | `aarch64-unknown-linux-gnu` |
| `x86_64-apple-darwin` | `x86_64-apple-darwin` |
| `aarch64-apple-darwin` | `aarch64-apple-darwin` |
| `wasm32-unknown-unknown` | `wasm32-unknown-unknown` |

These checks prove compilation and artifact generation. They do not prove execution or consumer linking on non-host targets. The host static-library symbol test passed 2/2, including with an external `CARGO_TARGET_DIR`; it is not a cross-target consumer link test.

## Additional recorded evidence

The resource-ceiling integration test now independently verifies exact bounded admission for queued channel values and background jobs:

- `cargo test -p zirk-runtime --test resource_ceilings -- --test-threads=1` — passed 2/2; accepts exactly 65,536 queued channel values and 256 supervisor jobs, then rejects the first excess enqueue/admission. This does not establish a global active-task, timer-registration, or managed-thread ceiling.

Two release benchmarks now cover runtime-only lock contention and blocking-pool admission in addition to the existing four benchmarks:

- `cargo test -p zirk-runtime --release --lib benchmark_ -- --ignored --nocapture --test-threads=1` — passed 2/2. On macOS ARM64 with Rust 1.94.1, cooperative mutex contention performed 2,048 acquisitions/sample with min/median/max 294,125 / 298,458 / 345,500 ns and checksum 14,336. Managed blocking-pool admission/drain performed 1,024 jobs/sample with 8 workers and a 1,024-job queue, at 578,500 / 1,058,125 / 1,360,750 ns and checksum 7,168. No memory ceiling was instrumented; neither benchmark measures a public Zirk API.

An independent read-only audit of `.settled()` confirmed the remaining cancellation-outcome gap is at the source API boundary, not in outcome decoding: lowering handles all four ABI statuses and roots/transfers result payloads, but direct pipeline member handles are private and no current source API can cancel one member while leaving the caller and siblings active. `Job.cancel()` cannot reach those internal handles. The missing requirement therefore needs a specified per-member cancellation capability and an E2E cleanup-join test; caller-cancellation and runtime child-cancellation tests do not prove that scenario.

- `cargo test -p zirk-runtime --lib combinators::tests::settled -- --test-threads=1` — passed 1/1 for runtime ordering and fulfilled/rejected outcomes; it does not generate an independently cancelled source pipeline member.

The managed `Concurrent.of(...).first()` source pipeline is now integrated and its focused native/lowering/runtime checks pass:

- `cargo test -p zirk-cli --test end_to_end concurrent_first_uses_completion_order_and_aggregates_failures -- --exact --test-threads=1` — passed 1/1; covers completion order, input-index tie, failure then success, all-failure suppression order, and cancellation-cleanup failure propagation.
- `cargo test -p zirk-ir --test lowering concurrent_first_lowers_to_completion_order_race_with_closed_abi_checks -- --exact --test-threads=1` — passed 1/1; verifies wait-any state machine, collect-all closure, loser cancellation/join and result draining under the cancellation shield.
- `cargo test -p zirk-runtime --lib executor::tests::opted_in_cancel_promotes_cleanup_failures_and_roots_suppressed_chain -- --exact --test-threads=1` — passed 1/1; verifies opt-in cleanup Throwable promotion and rooted suppression chain.
- `cargo test -p zirk-runtime --lib executor::tests::equal_completion_sequences_choose_the_lowest_input_index -- --exact --test-threads=1` — passed 1/1; verifies deterministic equal-sequence selection.

The direct source-level caller-cancellation path for `.first()` still lacks a native end-to-end trigger; cancellation lowering/runtime behavior is covered separately. This evidence closes task 3.2's stated requirements but does not close broader timeout or settled mixed-cancellation requirements.

## 2026-10-13 quota integration checkpoint (task 9.4)

The per-executor quota contract is 4,096 nonterminal tasks including the root and 4,096 occupied timer permits. Timer callbacks reserve both permits atomically; periodic jobs keep the timer permit between arms. This is not a process-wide OS-thread limit, and releasing an active-task permit does not itself reclaim an unconsumed terminal result record.

- `cargo test -p zirk-runtime --test task_timer_quota_abi -- --exact --nocapture` — passed 1/1; the checked ABI admits 4,095 children alongside the root, rejects the next admission with status 1 and no handle, then reuses capacity after termination.
- `cargo test -p zirk-runtime timer::tests::timer_job_reservation_is_atomic_and_reusable_for_periodic_arms -- --exact` — passed 2 matching tests; verifies atomic timer reservation, expiry/re-arm, cancellation and reuse.
- `cargo test -p zirk-runtime --quiet` — passed 568 tests, 7 ignored, across 17 suites.
- `cargo test -p zirk-ir --test lowering` — passed 165 tests; `cargo test -p zirk-codegen-llvm --test emission` — passed 58 tests, including checked spawn, sleep, `Timer.after` and `Timer.every` status paths.
- `cargo test -p zirk-cli --test end_to_end` — passed the then-current 59-test suite in 78.03s. The generated-Zirk quota regression below was added afterward and passed independently.
- `cargo test -p zirk-cli --test end_to_end concurrent_task_capacity_is_caught_and_reusable_from_zirk -- --exact --nocapture` — passed 1/1 in the main checkout. The fixture fills the executor to 4,096 active tasks including the root, catches `TaskCapacityError` on the next spawn, joins the accepted children, and then successfully admits a new child. Timer saturation is still covered at the runtime layer; generated-language timer saturation remains open.
- The requested generated-language `TimerCapacityError` saturation fixture was not added because the public contract cannot reach that branch safely: `MAX_ACTIVE_TASKS` and `MAX_ACTIVE_TIMERS` are both 4,096; the root counts toward active tasks; `Timer.after/every` atomically checks/reserves task capacity before timer capacity; and each ordinary armed sleep belongs to an active task and frees its timer slot before that task resumes. Thus the task ceiling dominates before a source program can request an additional timer at a full timer ceiling. Existing runtime test `executor::tests::timer_quota_rejection_does_not_publish_a_task_or_leak_either_permit` directly fills timer reservations and verifies status, no TCB/ready callback, unchanged task count, permit cleanup, and successful reuse; it is not generated-language evidence. Do not treat a source-level synthetic `throw TimerCapacityError(...)` as saturation coverage. A real CLI E2E needs a contract-approved way to make timer saturation independent of task saturation (distinct limits or a deterministic test-only quota seam).
- `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` — passed 1/1 after explicitly classifying the new fixture under the browser's existing `structured concurrency` capability exclusion.
- `cargo test --workspace --quiet` — passed 2,131 tests, 12 ignored, across 63 suites after integrating the generated-Zirk quota fixture and browser exclusion.
- `cargo clippy -p zirk-runtime -p zirk-ir -p zirk-codegen-llvm -p zirk-sema --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed. Clippy initially identified missing `# Safety` docs on four legacy ABI wrappers; the preconditions are now documented and the strict rerun passes.

Two read-only audits found no concrete race in the checked admission paths. They confirmed that completed task records can remain retained until consumed even after their active-task permits are released, and that the current task quota does not cover unmanaged host/runtime threads. Generated-language saturation E2E, the future managed `Thread.run` thread ceiling, and broader adversarial resource profiles remain open.

The cross-target ABI consumer check now additionally covers selected managed-completion and synchronization-foundation symbols:

- `bash scripts/validate-concurrency-abi-consumer.sh` — passed; Clang compiled a freestanding `wasm32-unknown-unknown` C consumer and `wasm-ld --no-undefined` linked 25 runtime symbols, including the new timer-race ABI. The generated Wasm module was not executed and does not prove runtime behavior or C ABI signature equivalence.
- `docker run --platform linux/amd64 ... debian:12` with Clang 14, LLD 14, and glibc multilib — passed; rebuilt runtime staticlibs for x86_64 and i686 and linked both final ELF consumers with 25 symbols and `--no-undefined`. Neither executable was run; links prove symbol resolution, not runtime behavior or C ABI signature equivalence.

Follow-up integration checks for completion capability boundaries also pass:

- `cargo test -p zirk-cli --test end_to_end phase_five_completion_apis_report_named_pending_gates -- --exact --test-threads=1` — passed 1/1 for the still-gated `Concurrent.each` and `Concurrent.each_settled` APIs. `.within(Duration)` is now enabled for supported `.first()` and `.settled()` pipelines.
- `cargo test -p zirk-sema --test typing completion_pipeline_terminals_validate_arguments -- --test-threads=1` — passed 1/1.
- `cargo test -p zirk-interpreter --lib cleanup_failure_cancellation_is_excluded_with_a_job_handles_diagnostic -- --test-threads=1` — passed 1/1; the new IR cancellation instruction fails closed with a named browser capability.
- `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` — passed 1/1 after listing all five native-only `.first()` fixtures under their observed capability gates.

The `Concurrent.each` contract was tightened while retaining its Phase 5 gate:

- `cargo test -p zirk-sema --test typing within_wrapped_first_and_settled_have_typed_pipeline_plans -- --exact --test-threads=1` — passed 1/1; mutable and inline collections fail the deep-Share `inmut::strict` requirement, a strict `List<MutableBox>` is rejected because its nested element graph is mutable, and a strict `List<Int32>` reaches the pending-feature gate without an ownership diagnostic.
- `cargo test -p zirk-cli --test end_to_end concurrent_each_collection_ownership_precedes_phase_gate -- --exact --test-threads=1` — passed 1/1; CLI fixtures verify mutable and inline lists report `E0482` alongside `E0424`, while a strict `List<Int32>` reports `E0424` without `E0482`.
- `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — passed 62/62 in 98.99 seconds after adding the three collection-ownership fixtures.
- `cargo test -p zirk-runtime --lib combinators::tests -- --test-threads=1` — passed 28/28; includes active-window tests proving both Rust reference wrappers use the approved default concurrency of 32 and preserve input order.
- `CARGO_NET_OFFLINE=true cargo clippy -p zirk-runtime --lib --tests -- -D warnings` — passed after adding the default-window constant and tests.
- `CARGO_NET_OFFLINE=true cargo clippy -p zirk-sema --all-targets -- -D warnings` — passed after the strict deep-Share collection check.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed. These checks validate formatting, whitespace, and OpenSpec structure; they do not establish Zirk-level `each` behavior.

The direct `Concurrent.of(...).settled()` compiler slice was validated after adding caller-cancellation shielding and exhaustive ABI-status handling:

- `cargo test -p zirk-ir --test lowering settled_pipeline_lowers_to_collect_scope_and_typed_outcome_transfer -- --exact --test-threads=1` — passed; checks stable job collection, typed outcomes, cancellation draining, shield ordering, and fail-closed unknown ABI statuses.
- `cargo test -p zirk-sema --test typing concurrent_settled_returns_ordered_list_of_generic_outcomes -- --exact --test-threads=1` — passed; confirms typed `List<Outcome<Int32>>`.
- `cargo test -p zirk-runtime --test job_outcome_take -- --test-threads=1` — passed 3/3; covers fulfilled/rejected/cancelled child outcomes, caller cancellation, GC roots, invalid handles/slots, and fail-closed misuse.
- `cargo test -p zirk-cli --test gc_generated_loop settled_outcome_preserves_nested_payloads_across_real_gc -- --exact --test-threads=1` — passed 1/1 with `ZIRK_GC_THRESHOLD=1` and 20,000 allocating iterations.
- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` — passed 1/1 over the valid corpus (56 fixtures), including stable settled order and returned-`Result.Error` versus thrown-`Throwable` behavior.
- `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` — passed 1/1 after the three settled fixtures were classified under the browser's explicit managed-completion capability exclusion.
- `cargo test -p zirk-ir -p zirk-codegen-llvm -p zirk-sema --quiet` — passed 921 tests across 12 suites after the exhaustive ABI-status branch was added.
- `cargo fmt --all -- --check` and scoped `git diff --check` — passed after the lowering and test updates.

The `.within(Duration)` source pipeline is now integrated through sema, IR, LLVM, and runtime:

- `cargo test -p zirk-cli --test end_to_end concurrent_within_first_and_settled_enforce_deadlines -- --exact --test-threads=1` — passed 1/1 across nine fixtures covering winner, timeout, cleanup join, caller cancellation, managed-result rooting, negative duration, and settled behavior.
- `cargo test -p zirk-codegen-llvm --test emission concurrent_within_emits_the_timer_wait_any_abi -- --exact --test-threads=1` — passed; generated LLVM declares and calls the separate timer ABI.
- `cargo test -p zirk-ir --test verification timer_wait_any_accepts_two_to_nine_jobs_only -- --exact --test-threads=1` — passed; verifies the 2..=9 compiler timer opcode bound and operand types while the regular opcode remains 2..=8.
- `cargo test -p zirk-runtime --test staticlib timer_wait_any -- --test-threads=1` and `cargo test -p zirk-runtime --test staticlib wait_any_abis_reject_excess_counts_before_reading_ids -- --exact --test-threads=1` — passed; the dedicated ABI accepts nine valid task handles and both ABIs reject invalid counts before reading the pointer.
- `cargo test -p zirk-interpreter --lib job_wait_any_with_timer_is_excluded_with_a_job_handles_diagnostic -- --test-threads=1` — passed; interpreter/browser preflight rejects the native scheduler instruction explicitly.
- `cargo test -p zirk-sema --test typing concurrent_of_rejects_void_results_for_typed_outcomes -- --exact --test-threads=1` — passed; `Concurrent.of(...).first()` and `.settled()` reject `Void` thunk results, matching the chosen `Outcome<T>` payload contract. The new invalid CLI corpus fixture is rejected by `cargo test -p zirk-cli --test end_to_end the_invalid_corpus_is_rejected_with_diagnostics -- --exact --test-threads=1` (1/1).

`tasks.md` records focused Miri, AddressSanitizer, and ThreadSanitizer runs, the four release benchmarks, and the earlier target/static-library checks. Those sanitizer and benchmark runs were not repeated during this final integrated validation. The executor uses `mmap(PROT_NONE)` guard pages, which prevents the stackful executor-root test from running under Miri. No Kani, Shuttle, Loom, or other exhaustive model checker is installed; deterministic generated model traces are covered by the runtime tests.

Remaining boundaries are tracked in `tasks.md`, including `Concurrent.each` / `Concurrent.each_settled` lowering, a source-level way to cancel an individual `Concurrent.of` member for `.settled()`'s `Cancelled` outcome, managed `Thread.run`, cooperative language synchronizers (runtime-only mutex/RwLock/semaphore/barrier/Once foundations are not public language support), broader resource observability, and site synchronization. The full change is not complete while those tasks remain open.

The earlier integrated-checkpoint fingerprint is stale after this slice; no new fingerprint is recorded until final integration. No commit or release synchronization was performed.

Cross-primitive synchronization cancellation stress was extended with deterministic queued-waiter cancellation in Semaphore, Barrier, and Once, plus a bounded five-second watchdog:

- `cargo test -p zirk-runtime --lib mixed_fifo_sync_contention_unwinds_and_conserves_waiters_and_permits -- --nocapture --test-threads=1` — passed 1/1; verifies cancelled waiter outcomes, live semaphore waiter recovery, broken barrier wakeup, no Once initialization residue, and zero remaining waiters/permits after the mixed stress.
- `cargo test -p zirk-runtime --lib sync_primitives::tests -- --test-threads=1` — passed 21 tests, including FIFO, cancellation, unwind, and conservation cases.
- `cargo clippy -p zirk-runtime --lib --tests -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.

The source-level collection combinators remain gated, but the native error type required by their invalid-input contract now has a generated-code construction/catch regression:

- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` — passed 1/1 over the complete valid corpus (62 fixtures); the new fixture constructs `InvalidArgumentError`, catches it by concrete type, and checks its code and message.

The bounded Rust runtime combinator now has an opt-in release benchmark. This does not measure the gated source-language `Concurrent.each` API:

- `cargo test -p zirk-runtime --test optimization_bench --release benchmark_bounded_each -- --ignored --nocapture` — passed 1/1 on macOS 27.0.1, `aarch64-apple-darwin`, Rust 1.94.1 / LLVM 21.1.8. For 256 inputs at limit 16 over 9 samples, min/median/max were 633,958 / 689,416 / 729,084 ns; checksum `87320318793856`; observed max active concurrency 16; all callbacks joined and zero remained active.

The native source-level `Concurrent.each` and `Concurrent.each_settled` vertical slice is now enabled. This checkpoint supersedes earlier entries that described these APIs as sema-gated:

- `cargo test -p zirk-sema --test typing within_and_collection_combinators_have_typed_pipeline_plans -- --exact --test-threads=1` — passed 1/1 after aligning valid fixtures with the selected `inmut::strict` deep-Share contract.
- `cargo test -p zirk-sema` — passed 641 tests across 3 suites.
- `cargo test -p zirk-ir --test lowering concurrent_each -- --test-threads=1` — passed 4 focused lowering tests for dynamic bounded admission, ordered typed staging, exhaustive outcome statuses, input/limit checks before admission, and default limit 32 including empty-input capping.
- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` — passed 1/1 over 61 runnable native corpus fixtures (75.21 seconds), including refill/order, fail-fast cleanup, invalid input before admission, and caller cancellation joining for both collection APIs.
- `cargo test -p zirk-cli --test gc_concurrent_each -- --test-threads=1` — passed 1/1 with `ZIRK_GC_THRESHOLD=1`; nested record/string payloads survive real collections in both `List<T>` and `List<Outcome<T>>`.
- `cargo test -p zirk-cli --test end_to_end concurrent_each_collection_requires_strict_deep_share_ownership -- --exact --test-threads=1` — passed 1/1; mutable and inline collection inputs are rejected while a strict deep-Share input type-checks.
- `cargo test -p zirk-cli --test end_to_end the_invalid_corpus_is_rejected_with_diagnostics -- --exact --test-threads=1` — passed 1/1 after removing the now-obsolete pending-feature invalid fixture.
- `cargo fmt --all` applied formatting after these changes. `openspec validate concurrency-completion --strict` passed before this validation entry was appended; rerun final format, diff, and OpenSpec checks after integration.

The browser interpreter deliberately excludes the native scheduler and managed-outcome instructions. Its instruction capability classifier recognizes `ScopeCancel` and `ScopeOutcomeWaitTake`, and the corpus exclusion matrix records their verified capability labels. This is a native-codegen implementation and does not claim browser execution.

Final integration for this slice:

- `cargo test -p zirk-ir --test lowering -- --test-threads=1` — passed 169/169.
- `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — passed 61/61 in 99.77 seconds.
- `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` — passed 1/1 in 11.32 seconds after classifying `ScopeCancel` as structured concurrency and `ScopeOutcomeWaitTake` as managed completion outcomes; all six native collection fixtures are explicitly excluded from browser execution for those capability limits.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after source, tests, and browser exclusion updates.

Per-branch `CancelledError` semantics for `Concurrent.of(...).settled()` are now implemented and supersede the earlier open item in this log:

- The lowering checks the rejected branch's rooted Throwable against the compiler-known `CancelledError` class. A match is retyped only after the runtime `IsInstance` check and staged as `Outcome.Cancelled`; other throwables remain `Rejected`. A separate outcome path ensures a `finally` cleanup error overrides cancellation as `Rejected`; ordinary caller cancellation still closes/joins the whole scope and returns no partial list.
- `cargo run -p zirk-cli --bin zirk -- run crates/zirk-cli/tests/corpus/valid/concurrent_settled_failure_channels.zrk` — passed. Output verifies cleanup before `Cancelled`, ordinary thrown failure as `Rejected`, successful `Fulfilled`, cleanup failure as `Rejected`, and returned `Result.Error` remaining a `Fulfilled` value.
- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --test-threads=1` — passed 1/1 over the complete valid native corpus (61 fixtures; 84.58 seconds).
- `cargo test -p zirk-ir --test lowering settled_pipeline_lowers_to_collect_scope_and_typed_outcome_transfer -- --exact --test-threads=1` — passed 1/1; `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` passed 1/1; `cargo test -p zirk-cli --test gc_generated_loop settled_outcome_preserves_nested_payloads_across_real_gc -- --exact --test-threads=1` passed 1/1.
- `cargo test -p zirk-ir --test lowering -- --test-threads=1` — passed 169/169 after the new cancellation classification and concrete payload retype.
- `cargo clippy -p zirk-sema -p zirk-ir -p zirk-interpreter -p zirk-browser -p zirk-cli --all-targets -- -D warnings` — passed after this implementation slice.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after the source, task, spec, and documentation updates.

Ownership-boundary and source-level performance follow-up:

- `cargo test -p zirk-sema --test typing concurrent_boundary_matrix_rejects_unproven_spawn_channel_and_parallel_values -- --exact --test-threads=1` — passed 1/1. It rejects mutable spawn captures, non-transferable spawn results, unsafe channel send/receive types, and non-transferable `parallel` results. Existing tests separately cover parallel mutable captures, timer callback captures/results, `Concurrent.of` captures, collection boundaries, and gated Thread.run entry/result checks.
- `cargo test -p zirk-cli --test concurrent_each_source_bench --release -- --ignored --nocapture` — passed 1/1. The benchmark compiles a strict `List<Int32>` of 256 elements, runs `Concurrent.each` at limit 16, warms up once, then records 11 execution samples. On macOS ARM64 (`aarch64-apple-darwin`), Rust 1.94.1, checksum `560000`; min/median/max were 2,454,791 / 3,184,042 / 3,700,416 ns. Compilation is excluded from samples and there are no timing assertions.

Background capacity contract follow-up:

- The user selected a distinct `BackgroundCapacityError` for saturation of the existing 256-active-job background supervisor. The delta spec now states that exact ceiling, typed rejection, and reuse after terminal completion. `BackgroundCapacityError` is registered as a compiler-known `RuntimeError` subclass; `native_capacity_errors_are_known_runtime_error_subclasses` now checks its hierarchy and construction/catch typing.
- `cargo test -p zirk-sema --test typing native_capacity_errors_are_known_runtime_error_subclasses -- --exact` — passed 1/1.
- `cargo test -p zirk-runtime --lib sync_primitives::tests::mixed_fifo_sync_contention_unwinds_and_conserves_waiters_and_permits -- --exact --nocapture` — passed 1/1 in isolation. The preceding `cargo test --workspace --quiet` run had failed this stress case at its five-second timeout while the full suite was running; the isolated rerun passed, so this remains a load-sensitive stress result rather than a reproducible failure.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after this contract/type update. At that historical checkpoint, `Concurrent.detach` remained gated pending language-level background failure delivery and `App.on_background_error(Throwable)` integration; source lowering and admission were implemented in a later slice.

Background quota and native error representation follow-up:

- Added a public-ABI integration test that holds all 256 background slots active, checks that status 1 rejects the 257th admission, releases and joins all 256 jobs, then retries until the public ABI admits another job. Polling is bounded and fails the test instead of hanging if cleanup or capacity reuse regresses.
- `cargo test -p zirk-runtime --test cancel_shutdown_gc_stress public_background_abi_enforces_quota_and_reuses_completed_slots -- --exact` — passed 1/1.
- Adding `BackgroundCapacityError` exposed an IR bug: sema knew its class, but native constructor lowering omitted the new exception identity. The native failure code table and class synthesis now include it, with lowering/codegen assertions for `E_BACKGROUND_CAPACITY`.
- `cargo test -p zirk-sema --test typing native_capacity_errors_are_known_runtime_error_subclasses -- --exact` — passed 1/1.
- `cargo test -p zirk-ir --test lowering a_repeated_literal_is_interned_once -- --exact` — passed 1/1, including the exact native failure code table size.
- `cargo test -p zirk-codegen-llvm --test emission task_and_timer_admission_statuses_are_checked_before_handle_use -- --exact` — passed 1/1, including emission of the new error code.
- `cargo test -p zirk-cli --test end_to_end background_capacity_error_is_constructible_and_catchable_from_zirk -- --exact` — passed 1/1; generated native code catches `BackgroundCapacityError` through `RuntimeError` and prints both `E_BACKGROUND_CAPACITY` and its message.

Gated `Concurrent.detach` semantic-boundary coverage:

- Sema now rejects incorrect arity, named/spread arguments, non-function values, callbacks with parameters, non-`Void` callback results, and mutable lambda captures before the feature gate. Valid callback shapes still report `E0424`; no runtime support was enabled.
- `cargo test -p zirk-sema --test typing concurrent_detach_ -- --test-threads=1` — passed 4/4.
- Source lowering, background `Throwable` delivery, and application-scope integration remain required before removing the gate.

After the sema diagnostic text was adjusted to preserve the existing Phase 5 message contract, the complete CLI end-to-end suite passed: `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — 62/62 in 91.67 seconds. This includes the new BackgroundCapacityError code/message fixture and all existing feature-gate diagnostics.

Admission-cause separation from the executor-wide task quota:

- A read-only audit found that `spawn_background` previously mapped both the 256-job supervisor ceiling and the executor's 4,096-task ceiling to the same admission error. The runtime now distinguishes them: C ABI status 1 means background capacity (`BackgroundCapacityError`), while status 4 means executor task capacity (`TaskCapacityError`). Drain and internal ID statuses retain their separate codes.
- Added runtime regressions for the private executor error distinction and the public C ABI status 4 path. These prevent reporting the wrong resource error if the general executor limit is reached before the background-specific limit.
- `cargo test -p zirk-runtime --lib background_ -- --test-threads=1` — passed 11/11 runtime background tests, including both the internal distinction and public status-4 ABI case.
- `cargo test -p zirk-runtime --test cancel_shutdown_gc_stress public_background_abi_enforces_quota_and_reuses_completed_slots -- --exact` — passed 1/1 after bounded polling was added to ensure a regression fails instead of hanging.
- A second read-only audit found that failed background admission consumed an internal monotonically increasing job ID. The executor now commits the next ID only after supervisor admission succeeds; the quota unit test asserts rejected admission leaves the ID counter, background task map, and active-task count unchanged.
- `cargo test -p zirk-runtime --lib background_admission_is_bounded_before_tasks_become_ready -- --nocapture` and `cargo test -p zirk-runtime --lib background_admission_distinguishes_executor_task_capacity -- --nocapture` — passed 1/1 each.
- `cargo test -p zirk-runtime -- --test-threads=1` — passed the full runtime package (unit, integration, ABI, stress, and doc-test targets). The unit suite reports 328 passed and 2 ignored; all runnable integration suites passed, including the 256-job public ABI quota test.
- `cargo clippy -p zirk-runtime -p zirk-sema -p zirk-ir -p zirk-codegen-llvm -p zirk-cli --all-targets -- -D warnings` — passed after the admission status, ID accounting, and sema diagnostic updates.

### Runtime-configurable background shutdown grace

- Exposed `Executor::with_background_shutdown_grace(Duration)` as a runtime-level configuration point; the default constructor continues to use the five-second constant. Its API docs state that expiry reports unfinished work but cannot preempt non-yielding work or release roots while participants may still access the heap.
- Added `configured_background_grace_allows_cooperative_cleanup`, alongside the existing zero-grace expiry case.
- `cargo test -p zirk-runtime --lib executor::tests::configured_background_grace_allows_cooperative_cleanup -- --exact --test-threads=1` — passed 1/1.
- The source-level `App.shutdown_grace`, host signal policy, and stuck non-cooperative participant termination remain unimplemented; this runtime builder does not close task 4.3.

### Timer cancellation and callback result coverage

- Added sema regressions proving `Timer.every` rejects a callback returning `Int32`, named function values, and parameterized lambdas, with diagnostics identifying the required callback contract. Focused command passed: `cargo test -p zirk-sema --test typing timer_callbacks_enforce_capture_and_result_ownership_boundaries -- --exact` (1 passed).
- Added a 32-cycle public runtime stress case for periodic timer cancellation with a GC-captured callback. It forces allocation pressure while the callback is active, then cancels and drains the scope. Parent-checkout rerun `cargo test -p zirk-runtime --test cancel_shutdown_gc_stress -- --test-threads=1` passed 4/4. This proves captured data stays valid while active and shutdown returns, not a direct post-shutdown root-count baseline.

### Integrated workspace rerun after these changes

- `cargo test --workspace -- --test-threads=1` — passed **2,186 tests, 14 ignored, across 66 suites** (test harness time 141.13 seconds). This current-checkout serial run includes the configured-grace unit regression and timer cancellation stress test.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after the runtime/API, test, and task-evidence updates.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed after replacing a range-index loop in the new timer stress test with `enumerate()`.

### Public task quota release after cancellation

- Added a public checked-ABI stress regression that occupies all 4,095 child slots with timer-suspended tasks, cancels every task through `zirk_rt_cancel`, and verifies a timer-backed job can be admitted again. It then refills the task quota with suspended jobs, cancels them, verifies timer-backed admission recovers again, and checks ordinary task admission after cleanup.
- `cargo test -p zirk-runtime --test task_timer_quota_abi -- --test-threads=1` — passed 2/2, including normal-completion reuse and cancellation cleanup/reuse across both task and timer permits.
- This adds public-ABI evidence for both task and timer quota release under cancellation; it does not close 9.4, whose remaining gaps include managed OS-thread ceilings and independent language-level timer saturation.

### Collector progress with a mutator in native state

- Strengthened `blocked_native_mutator_keeps_stable_roots_visible_to_collection` so the stop-the-world coordinator runs on another thread and the test asserts, with a one-second bounded receive, that collection completes while a participant remains in `InNative` with its stable roots registered.
- `cargo test -p zirk-runtime --lib collector::stw::tests::blocked_native_mutator_keeps_stable_roots_visible_to_collection -- --exact --test-threads=1` — passed 1/1.
- This proves bounded collector progress and stable-root retention for this native-state scenario; it does not prove arbitrary FFI callbacks or managed `Thread.run` are safe.

### Background shutdown disarms a suspended timer

- Added `root_completion_disarms_suspended_background_timer_before_drain_finishes`: the root waits until a background task is suspended on a 60-second timer, then returns. Cancellation cleanup checks that the timer registration has returned to its baseline before setting the completion flag. The executor uses a 250 ms shutdown grace.
- `cargo test -p zirk-runtime --lib executor::tests::root_completion_disarms_suspended_background_timer_before_drain_finishes -- --exact --test-threads=1` — passed 1/1 (agent run in the shared checkout).
- The assertion proves timer deregistration before drain completion for this cooperative path. It does not measure the post-shutdown task/root baseline or cover host signals and service panics; task 4.6 remains partial.

### Integrated validation after quota and lifecycle regressions

- `cargo test --workspace -- --test-threads=1` — passed **2,192 tests, 14 ignored, across 66 suites** (142.25 seconds). This includes both the checked-ABI quota release after cancelling 4,095 timer-suspended children and the root-completion timer-deregistration regression.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.
- At the time of this earlier run, source-level `Atomic<T>` was not yet implemented. Later validation is recorded below. Host signals and `Thread.run` remain unavailable, and the run does not close the remaining partial checklist items.

### Semantics coverage for gated thread and timer callback effects

- Added sema cases for `Thread.run` rejecting direct channel/timer waits and transitive suspend, block, spawn, and unknown-call effects, while pure valid-shape callbacks retain the Phase 5 diagnostic only.
- Added effect-summary cases showing `Timer.after` registration itself does not suspend the caller, while the callback's transitive suspend/block/spawn/unknown effects remain attributed to the callback summary.
- `cargo test -p zirk-sema --test typing -- --test-threads=1` — passed 615 tests.
- `cargo fmt --all -- --check` and `git diff --check` passed in the agent workspace; parent checkout will run the integrated format/OpenSpec checks before the next workspace validation.
- These tests strengthen sema evidence for tasks 2.5 and 9.1. They do not enable `Thread.run` or prove runtime thread safety; ownership and synchronizer guard matrices remain open.

### Atomic operation-order contract clarification

- Clarified per-operation order legality: `load` permits relaxed/acquire/SeqCst; `store` permits relaxed/release/SeqCst; read-modify-write permits all orders; compare-exchange preserves its success/failure constraints.
- User selected `InvalidArgumentError` before payload access for a dynamically supplied incompatible order. Statically known invalid orders remain compile-time diagnostics.
- Updated the synchronization delta, design D9, ADR-023, and task 7.3. This is a contract update only; source `Atomic<T>` is still gated, so no source/runtime behavior is claimed.
- `openspec validate concurrency-completion --strict` and `git diff --check` — passed for the contract-only update.

The runtime reference surface was then extended with explicit order parameters for load, store, exchange, and integer fetch operations. It rejects illegal load/store orders before touching the cell and accepts the full ordering set for read-modify-write operations. `cargo test -p zirk-runtime --lib atomic::tests -- --test-threads=1` — passed 9/9, including operation-order legality and unchanged payload on rejected stores. `cargo clippy -p zirk-runtime --lib -- -D warnings` — passed. The full `cargo test -p zirk-runtime -- --test-threads=1` run passed **614 tests, with 8 ignored, across 18 suites**. After the final runtime and spec edits, `cargo test --workspace -- --test-threads=1` passed **2,193 tests, with 14 ignored, across 66 suites** (145.74 seconds). This is Rust `AtomicCell` reference behavior; dynamic invalid ordering is not yet mapped to a Zirk `InvalidArgumentError`, and generated LLVM/source `Atomic<T>` remains unimplemented.

After the explicit-ordering change, `cargo check -p zirk-runtime --tests --target <target>` and `cargo build -p zirk-runtime --target <target>` both passed for all ten installed targets: `aarch64-apple-darwin`, `aarch64-pc-windows-msvc`, `aarch64-unknown-linux-gnu`, `armv7-unknown-linux-gnueabihf`, `i686-pc-windows-msvc`, `i686-unknown-linux-gnu`, `wasm32-unknown-unknown`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, and `x86_64-unknown-linux-gnu`. `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` also passed. These checks compile and build artifacts only; no cross-target executable was run. The result does not close task 7.3 or implement source `Atomic<T>`.

### Managed source Atomic<T> vertical slice

- Added GC-inline Atomic IR operations, exact LLVM calls to `zirk_rt_atomic_managed_*`, rooted-object verification, source lowering for construction and all operations, and Result<T,T> lowering for both compare-exchange outcomes. The legacy Box-owned C ABI remains separate.
- `cargo test -p zirk-sema --test typing atomic_ -- --test-threads=1` — passed 10 tests. These cover scalar admission, unsupported payloads, type/arity errors, unsafe ordering requirements, operation-specific constraints, and compare-exchange ordering.
- `cargo test -p zirk-ir --quiet` — passed 231 tests. `cargo test -p zirk-codegen-llvm --quiet` — passed 74 tests. Both include managed Atomic verifier/emission coverage.
- `cargo test -p zirk-runtime --lib atomic::tests -- --test-threads=1` — passed 11 tests. `cargo test -p zirk-runtime --test atomic_compare_exchange_order_matrix -- --test-threads=1` — passed the full runtime order-pair matrix.
- The generated Zirk Atomic fixture ran successfully and verified load/store/exchange/fetch-add/fetch-sub, successful and failed compare-exchange, valid Release/Acquire operations inside `unsafe`, dynamic invalid-order conversion to `InvalidArgumentError`, and unchanged payload after rejection.
- `cargo test -p zirk-cli --test gc_generated_loop managed_atomic_remains_rooted_across_real_gc -- --exact --test-threads=1` — passed 1 test with `ZIRK_GC_THRESHOLD=1` and 20,000 allocating loop iterations.
- `cargo check -p zirk-runtime --lib --target <target>` — passed for all nine LLVM codegen triples, mapping `i386-unknown-linux-gnu` to Rust `i686-unknown-linux-gnu`, and for `wasm32-unknown-unknown`. These are compile checks; no non-host binaries were executed.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after the IR/LLVM/lowering changes and Atomic task updates.
- `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — passed 66/66 tests in 97.59s. Its valid-corpus test accepted and executed all 251 valid fixtures; its invalid-corpus test rejected all 191 invalid fixtures with diagnostics.

### Managed Atomic browser boundary and worker-root GC stress

- `cargo test -p zirk-interpreter --lib` — passed 57/57 tests after adding explicit browser exclusions and defensive handling for managed Atomic, background work, and completion-order scheduler IR.
- `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` initially identified four missing expected exclusions (two managed Atomic fixtures and two `Concurrent.detach` fixtures). Added those capability entries, then the same command passed 1/1.
- Added `gc_worker_root_stress`: four registered pool workers each retain a managed object in a shadow-root slot while allocating 2,048 objects under `ZIRK_GC_THRESHOLD=1`. It also parks a rooted executor task, forces GC pressure, cancels the task, and verifies the object remains readable during cleanup before scope join. `cargo test -p zirk-runtime --test gc_worker_root_stress -- --test-threads=1` passed 1/1 after the fixture was corrected to preserve the collector's three-word allocation header and root the address of the reference slot.
- `cargo test --workspace -- --test-threads=1` passed 2,223 tests, with 14 ignored across 70 suites. Subsequent Atomic checker cleanup and test cleanup passed `cargo test -p zirk-sema --test typing atomic_ -- --test-threads=1` (10 passed), `cargo test -p zirk-sema --test typing concurrent_detach -- --test-threads=1` (4 passed), and the focused `application_spawn_service` pending-gate CLI test (1 passed).
- After those edits, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` passed. Cross-target generated execution and native stress remain unverified. The later `gc_worker_root_stress` extension also passed `cargo clippy -p zirk-runtime --test gc_worker_root_stress -- -D warnings` and the full `cargo test -p zirk-runtime -- --test-threads=1` run (618 passed, 8 ignored, 20 suites).

### Integrated verification refresh — 2026-10-13

- `cargo test --workspace -- --test-threads=1` — passed **2,234 tests, 14 ignored, 70 suites** in 149.34 seconds.
- The integrated CLI end-to-end suite passed **66/66**, including the full corpus: 253 valid fixtures and 189 invalid fixtures, plus `Thread.run` result/Throwable E2E cases.
- `cargo test -p zirk-sema --test typing -- --test-threads=1` — passed 625 tests. The regression `concurrent_protect_rejects_blocking_unknown_and_spawning_cleanup_effects` now checks that the blocking effect of `Thread.run` reaches `Concurrent.protect` both directly and through a named function. The focused test passed 1/1.
- `cargo test -p zirk-codegen-llvm --lib managed_thread -- --test-threads=1` — passed 3 tests: object emission for six supported 64-bit triples and target-gate rejection on Wasm/i386/armv7. `cargo test -p zirk-codegen-llvm --lib tests::atomic_cell_abi_emits_exact_external_symbols_and_opaque_pointer_signatures -- --exact --test-threads=1` — passed 1 test across the nine LLVM target matrix.
- `cargo test -p zirk-runtime --lib managed_blocking::tests::cancellation_joins_active_managed_callback_across_gc_pressure_with_roots_live -- --exact --test-threads=1` — passed 1 test. It combines a worker-side real collection, rooted capture/result checks, cancellation while the callback is active, join-before-return, and a follow-up collection after worker unregistration.
- `scripts/validate-concurrency-abi-signatures.sh` — passed all 10 installed Rust targets, matching 25 lowered Rust/C ABI signatures on each. It linked the Wasm consumer with undefined symbols disallowed; the other nine targets were signature checks without a consumer link, and no target was executed. `python3 scripts/test_concurrency_abi_signatures.py` passed 4 tests, including missing/duplicate symbols and type-width drift; `bash -n scripts/validate-concurrency-abi-signatures.sh` passed.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` passed. Cross-target runtime execution, non-Wasm C consumer linking, and formal exhaustive model checking remain unproven.

## 2026-10-13 follow-up: managed thread admission and GC shutdown

- `cargo test -p zirk-runtime --lib managed_thread_admission_stays_within_worker_and_queue_ceilings -- --test-threads=1` — passed. The test saturates the singleton managed `Thread.run` worker pool and its 1,024-entry queue, verifies cooperative admission retries do not create workers past the configured ceiling, releases the callbacks, and checks all callers and mutator registrations drain at shutdown.
- `cargo test -p zirk-runtime --lib cancellation_joins_active_managed_callback_across_gc_pressure_with_roots_live -- --test-threads=1` — passed after adding a test-only participant-registry oracle; verifies cancellation waits for a worker-triggered collection and callback completion, then checks the pool shutdown leaves no registered mutator.
- `cargo test -p zirk-runtime --lib blocked_native_mutator_keeps_stable_roots_visible_to_collection -- --test-threads=1` — passed; collection completes while a registered participant remains in the stable `InNative` state.
- `cargo test -p zirk-ir --test lowering ordinary_loop_back_edges_receive_one_poll -- --exact --test-threads=1` and `continue_back_edges_receive_one_poll_without_duplicate_edges` — passed; ordinary and `continue` back edges retain safepoint polls.
- `cargo test -p zirk-sema --test typing thread_run_rejects_direct_channel_and_timer_suspension_effects -- --exact --test-threads=1` and `cargo test -p zirk-cli --test end_to_end thread_run_checks_effects_and_accepts_complete_callbacks -- --exact --test-threads=1` — passed; direct/transitive suspension, spawn, and unsafe/incomplete extern effects remain rejected at the managed-thread boundary.
- `cargo test -p zirk-cli --test end_to_end ownership_boundary_corpus_covers_each_exercisable_api_boundary -- --exact --test-threads=1` — passed across 12 ownership fixtures. `cargo test -p zirk-sema --test typing mutex_and_rwlock_guard_apis_remain_phase_gated -- --exact --test-threads=1` — passed; source synchronization guards remain unavailable until their ownership/effect model is implemented.
- `python3 scripts/test_concurrency_abi_signatures.py` — passed 6 tests, including a negative nested-callback signature drift check for `zirk_rt_spawn`. The cross-target signature validator passed all 10 installed Rust targets; its C consumer linked on Wasm, with existing Linux x86_64/i686 link checks retained separately. Other targets were not claimed as linked or executed.

## 2026-10-13 follow-up: queued cancellation, ABI signatures, and release metrics

- `cargo test -p zirk-runtime --lib managed_thread_admission_stays_within_worker_and_queue_ceilings -- --test-threads=1` — passed. Besides filling all workers and queue slots, the test cancels an already-admitted queued caller before its worker callback starts, admits one additional caller through cooperative retry, then releases and joins all callbacks.
- `cargo test -p zirk-runtime --release --lib benchmark_managed_thread_run_admission -- --ignored --test-threads=1` — passed. Direct harness output: macOS ARM64, Rust 1.94.1, 7 samples of 1,024 joined callbacks, min/median/max 6,222,541 / 6,331,875 / 6,652,917 ns, checksum 7,168, 18 persistent workers, queue cap 1,024, total process thread ceiling 19, process lifetime peak RSS 3,702,784 bytes. This RSS is process-wide peak resident memory, not per-callback allocation.
- Release benchmark reporters now include process peak RSS on macOS/Linux and explicit thread/queue ceilings where known; unsupported RSS platforms print `unavailable`.
- `cargo test -p zirk-runtime --test concurrency_bench --no-run` and `cargo test -p zirk-runtime --test optimization_bench --no-run` — passed after adding the shared process RSS reporter.

## Ownership boundary closure

- `cargo test -p zirk-cli --test end_to_end ownership_boundary_corpus_covers_each_exercisable_api_boundary -- --exact --test-threads=1` — passed all 12 boundary fixtures. The cases reject mutable captures, non-transferable values, unsafe channel boundaries, and aliased parallel inputs before code generation.
- `cargo test -p zirk-sema --test typing mutex_and_rwlock_guard_apis_remain_phase_gated -- --exact --test-threads=1` and the pending-type CLI corpus confirm Mutex, RwLock, Semaphore, Barrier, and Once constructors stay behind `E0402`; no source handle can reach an unchecked ownership boundary. Synchronization implementation tasks remain open separately.

### Managed callback cancellation and GC cross-platform correction

- The focused cancellation/GC regression originally reported success because its assertions ran inside the caller task and cancellation suppressed the panic. Moving observations outside the cancelled task exposed an invalid test action: it called the worker-local collector directly, which cannot walk executor task roots. The test now forces allocation-triggered stop-the-world collection from the registered worker, then checks observations after caller cancellation, worker join, and another collection after worker unregistration.
- `cargo test -p zirk-runtime --lib cancellation_joins_active_managed_callback_across_gc_pressure_with_roots_live -- --test-threads=1` — passed 1/1 after correction.
- `cargo test -p zirk-runtime --lib managed_blocking::tests -- --test-threads=1` — passed 13, ignored 2.
- `cargo test -p zirk-runtime --test gc_worker_root_stress -- --test-threads=1` — passed 1/1.
- The focused cancellation/GC test passed on macOS ARM64 (native), macOS x86_64 (Rosetta), Linux x86_64 (Docker amd64 emulation), and Linux ARM64 (Docker). Emulated results are identified as such; this is not evidence from four independent hardware architectures.
- The regression now supports task 9.3. Task 5.6 remains partial until blocked-FFI transition coverage and all remaining supported-target executions are verified.

## 2026-10-13 Mutex scoped-view prerequisite audit

- ADR-023 and the type-system delta now make the missing source-level prerequisite explicit: `Mutex<T>.with` needs an internal scope-indexed borrowed-view type, transitive non-suspension validation, rooted callback/handle lifetime across scheduler wait, and a per-object GC-traced payload. An ordinary callback parameter of type `T` is insufficient because it can escape. No source gate was removed and no native-handle registry was introduced.
- `cargo test -p zirk-sema --test typing mutex_and_rwlock_guard_apis_remain_phase_gated -- --exact --test-threads=1` — passed 1/1; Mutex and RwLock remain unavailable pending the scoped-view model.
- `cargo test -p zirk-cli --test end_to_end pending_concurrency_types_report_the_phase_gate -- --exact --test-threads=1` — passed 1/1; Thread, Mutex, RwLock, Semaphore, Barrier, and Once still report the Phase 5 gate.
- `cargo check -p zirk-runtime --lib` — passed. `openspec validate concurrency-completion --strict` and `git diff --check` — passed.

## 2026-10-13 Scoped-view representation feasibility

- Inspected `crates/zirk-sema/src/types.rs`, `scope.rs`, and the lambda, assignment, return, and capture paths in `checker.rs`, plus `crates/zirk-ir/src/ir.rs` and `lower.rs`. Sema's `Type` is a `Copy { base, nullable }` value; `Base` has no region/borrow variant or per-callback region table. Lambda parameters are declared as ordinary `Binding { ty: Type }`, and function identity is signature/layout based. Capturing a binding only records its ordinary type; it does not constrain the lifetime of a nested closure. `expect_assignable` and IR lowering operate on the referent's ordinary value type.
- IR's `IrType` has no borrowed capability/region. Its typed slots, callable captures, aggregates, fields, and return values represent ordinary values and heap references. There is no callback-region identity or verifier use/escape analysis to reject a value that is copied into a field, aggregate, return slot, or escaping closure. Consequently, adding only a sema marker or a `Mutex.with` lowering would erase the lifetime proof before the portable IR verification boundary; accepting either would be unsound.
- The minimum architecture is now made concrete in ADR-023: interned scope-identity plus referent in sema; explicit propagation through type acceptance/inference/substitution/capture; dedicated region-bearing IR type and verifier-enforced non-escape use rules; callback effect/no-suspend proof; rooted mutex/callback across queued acquire; and unlock on every exit edge. The IR/package schema version must change if that representation changes its serialized form.
- No production code or Phase 5 gate was changed because this prerequisite cannot be implemented soundly as a local API patch; the compiler currently lacks the region identity and IR verifier invariants themselves. Existing regression checks preserve the fail-closed behavior:
  - `cargo test -p zirk-sema --test typing mutex_and_rwlock_guard_apis_remain_phase_gated -- --exact --test-threads=1` — passed, 1/1.
  - `cargo test -p zirk-cli --test end_to_end pending_concurrency_types_report_the_phase_gate -- --exact --test-threads=1` — passed, 1/1 on 2026-10-13. This recheck supersedes the earlier local compile failure note; the cited `Base::ServiceContext` match arm is absent from the current checkout.
- `cargo test -p zirk-runtime --lib sync_mutex::tests -- --test-threads=1` — passed, 9 tests with 1 ignored on 2026-10-13. This recheck supersedes the earlier E0373 compile failure note.

### Native shutdown-grace configuration

- `cargo test -p zirk-cli --test end_to_end app_shutdown_grace_is_configurable_from_native_zirk_and_negative_literals_are_rejected -- --exact --test-threads=1` — passed 1/1 after the final runtime guard was added. The native API accepts zero grace, rejects negative values with `InvalidArgumentError`, and rejects updates after drain begins.
- `cargo test -p zirk-browser --test corpus_parity -- --test-threads=1` — passed 3/3 after adding an explicit browser exclusion and user-facing capability label for `AppSetShutdownGrace`.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.
- Host signal policy and non-cooperative/native participant lifetime protocol remain open under task 4.3.
- The first parity run found that `instruction_support` excluded `AppSetShutdownGrace` but the diagnostic mapper lacked its `managed background work` label. Added the missing label and an interpreter unit regression; final corpus parity passes.

### Workspace verification after GC and shutdown-grace integration

- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,239 tests, with 15 ignored, across 70 suites** (150.78 seconds). This includes the final App shutdown-grace runtime guard, browser capability mapping, and corrected cancellation/GC regression.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed after removing a same-type cast in the new managed-thread benchmark. `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` also pass on this state.
- Re-ran `collector::stw::tests::blocked_native_mutator_keeps_stable_roots_visible_to_collection` on the same four environments; it passed 1/1 on each. Windows 64-bit target execution remains unverified, so task 5.6 stays partial.

### Managed callback drain-expiry lifetime regression

- Added `crates/zirk-runtime/tests/managed_callback_shutdown_lifetime.rs`: a detached caller enters `Thread.run`; the callback allocates and roots a managed object, then remains active beyond a 1ms background grace while the executor forces GC at threshold 1. After release, it publishes the object into the caller's rooted result slot; the regression checks its payload and asserts `run_main` returns only after callback completion.
- `cargo test -p zirk-runtime --test managed_callback_shutdown_lifetime -- --test-threads=1` — passed 1/1.
- `cargo test -p zirk-runtime --lib executor::tests::expired_background_grace_reports_and_stops_before_unstarted_work_runs -- --exact --test-threads=1` — passed 1/1 after removing never-started canceled tasks from the ready queue before terminal completion.
- `cargo test -p zirk-runtime --test cancel_shutdown_gc_stress -- --test-threads=1` — passed 4/4 after fixing successful/cancelled background completion to release the supervisor admission slot.
- `cargo test -p zirk-runtime --quiet -- --test-threads=1` — passed 630 tests, 9 ignored across 21 suites. `openspec validate concurrency-completion --strict` passed.
- Shutdown now retains heap/TCB/caller-slot lifetime until managed callbacks stop. A native callback that never returns can therefore keep `run_main` alive after the grace deadline; bounded host termination policy and signal integration remain open under 4.3.
- `cargo check -p zirk-runtime --tests --target x86_64-pc-windows-msvc` — passed (2 crates compiled). This is cross-target type/build evidence only; no Windows executable was run. A local Windows ARM VM has no accessible guest session, and no Windows x64 runner is available in this session, so 5.6 remains open.

### `App.on_background_error` managed dispatch

- The `(Throwable): Void` source handler is lowered into a managed registration. The executor roots the handler capture and pending `Throwable`, invokes the callback outside the mutable executor borrow, and contains handler failures without recursively dispatching them.
- `cargo test -p zirk-cli --test end_to_end managed_background_error_handler -- --test-threads=1` — passed 2/2. The tests cover delivery through a forced GC with a managed capture and thrown `Throwable`, plus a failing handler whose diagnostic appears once while the process completes successfully.
- `ZIRK_GC_THRESHOLD=1 cargo test -p zirk-runtime --test managed_callback_shutdown_lifetime -- --test-threads=1 --nocapture` — passed 1/1; the callback's rooted object survives collection and is published before `run_main` returns.
- `cargo clippy -p zirk-runtime -p zirk-cli --all-targets -- -D warnings` — passed. These checks close task 4.4; they do not establish host signal handling or hard termination for a non-cooperative native callback, which remain under 4.3.

### Final workspace verification after managed background-error dispatch

- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,249 tests, with 15 ignored, across 71 suites**. This run includes managed `App.on_background_error` source-to-runtime delivery, forced-GC capture/Throwable handling, handler-failure containment, browser capability exclusions for native fixtures, and the `ServiceContext` fail-closed source-type regression.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed. `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed on the same checkout.
- Focused validation also passed: `cargo test -p zirk-cli --test end_to_end managed_background_error_handler -- --test-threads=1` (2/2), `ZIRK_GC_THRESHOLD=1 cargo test -p zirk-runtime --test managed_callback_shutdown_lifetime -- --test-threads=1 --nocapture` (1/1), and `cargo test -p zirk-browser --test corpus_parity native_corpus_runs_in_the_browser_or_is_explicitly_excluded -- --exact --test-threads=1` (1/1).
- `application.spawn_service` remains behind the Phase 5 gate; runtime supervisor bookkeeping tests do not prove language-level startup, readiness, or ordered shutdown.

## Shutdown deadline host policy (2026-10-13)

- `cargo test -p zirk-runtime --test shutdown_deadline_cli -- --test-threads=1` — passed 1/1. The child process parks a managed native callback forever, configures zero grace, and the parent verifies exit code 124 plus the unfinished-background diagnostic. This confirms process-level termination while live participant roots remain intact until the OS tears down the process.
- `cargo test -p zirk-runtime --test staticlib the_lifecycle_symbols_carry_no_mangling -- --exact` — passed after refreshing the static library; verifies unmangled `zirk_rt_run_main`, `zirk_rt_run_main_cli`, and `zirk_rt_run_main_with_deadline_policy` exports. The test now chooses the newest hashed staticlib when multiple historical artifacts exist.
- `cargo test -p zirk-codegen-llvm --test emission the_entrypoint_wraps_main_with_the_runtime_lifecycle -- --exact` — passed; generated CLI `main` uses the CLI deadline-policy entrypoint.
- `cargo test -p zirk-codegen-llvm --test target_matrix -- --test-threads=1` — passed 2/2; all configured target objects still emit.
- `python3 scripts/test_concurrency_abi_signatures.py` — passed 6/6. This checks the existing concurrency ABI signature matrix; the lifecycle symbol exports are separately checked by the staticlib test.
- Host signal delivery remains host-owned; 4.6 retains signal lifecycle stress coverage. These checks do not establish external OS signal forwarding or graceful interruption of a non-cooperative task.
- `ZIRK_ABI_TARGETS=wasm32-unknown-unknown scripts/validate-concurrency-abi-signatures.sh` — matched all 27 lowered Rust/C signatures and linked the Wasm consumer with `--no-undefined` (link only; not executed). This includes the generated CLI runner and explicit host deadline-policy selector.
- `cargo test -p zirk-cli --test end_to_end the_valid_corpus_compiles_and_produces_the_expected_output -- --exact --nocapture` — passed over the complete valid corpus after adding the `.exit` sidecar for the zero-grace fixture (exit 124 is now checked as the specified outcome).
- `cargo test -p zirk-runtime --lib tests::shutdown_releases_executor_heap_tasks_timers_and_gc_participants -- --exact --nocapture` — passed 1/1; verifies the heap publication and registered GC participant set are empty after joining managed work. Executor shutdown also asserts zero active task/timer permits and no armed timers in debug builds.
- `cargo test --workspace --quiet -- --test-threads=1` — passed the complete workspace after the shutdown-policy and invariant changes: **2,250 passed, 15 ignored, 71 test suites**. This rerun includes the deadline exit expectation, the runtime shutdown heap/participant baseline assertion, and debug checks for terminal tasks and released timer/task permits.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed on the same source state; `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` also passed.

## Host signal delivery and CLI forwarding follow-up (2026-10-13)

- `cargo test -p zirk-runtime --quiet -- --test-threads=1` — passed **674 tests, 9 ignored, across 24 suites**. This includes the host notification first-wins tests and a concurrent stress test with eight requesters over 64 reset/consume rounds.
- `cargo test -p zirk-runtime --test cli_os_signals -- --test-threads=1` — passed **5/5**. Subprocess tests verify Unix SIGINT cleanup/status 130, SIGTERM cleanup/status 143, inherited SIG_IGN, hard deadline/status 124 for a non-yielding root, and eight repeated SIGINT child lifecycles with cleanup.
- `cargo test -p zirk-cli --lib driver::signal_forwarding_tests -- --test-threads=1` — passed **4/4**. The CLI forwards parent-only SIGINT to its direct child, preserves child status, retains inherited SIG_IGN, and unregisters its process-global signal hook on return. Error paths kill and reap the child if status polling fails.
- `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — passed **71/71**.
- `cargo check -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc` and `cargo clippy -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc -- -D warnings` — passed, including the Windows console callback for Ctrl-C/Ctrl-Break and its gated mapping test. These are cross-target compile/lint checks, not execution evidence.
- `cargo check -p zirk-cli -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc` — could not complete locally: `llvm-sys` compiled its C wrapper with the macOS SDK and failed because `inttypes.h` was unavailable. Windows runner execution is still required for task 5.6.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after these changes.

## GC participant transition assertions (2026-10-13)

- Stop-the-world participant lookup now fails immediately if `park`, blocking-native transitions, or unregister receives an unknown participant ID. Resuming from native execution also asserts that the participant is still registered and in `InNative`; unregister asserts that its registration was removed exactly once.
- `cargo test -p zirk-runtime --lib collector::stw::tests -- --test-threads=1` — passed 6/6, including native reentry, collection-time unregister, and root survival through a registered native participant.
- `cargo test -p zirk-runtime --lib -- --test-threads=1` — passed 351 tests, 3 ignored.
- `cargo clippy -p zirk-runtime --lib -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.

## Service lifecycle contract review (2026-10-13)

- A source-level service is required, depends on every earlier required service, and starts as soon as registration makes it eligible. Registration is available from managed application tasks until drain begins. Returning normally or throwing after readiness is an unexpected runtime failure. A handler return does not end the application; `main` completion or host shutdown does. Services share `App.shutdown_grace` with background jobs.
- The contract was subsequently implemented end-to-end: source sema, IR verification/lowering, LLVM callback thunk, typed lifecycle errors, and runtime ABI/supervisor. See the service integration evidence below.
- `openspec validate concurrency-completion --strict` — passed after the contract update.

## Source-level application services and browser capability boundary (2026-10-13)

- `cargo test -p zirk-sema --test typing -- --test-threads=1` — passed 630 typing tests for service callback shape, `ServiceContext`, readiness scope, and ownership.
- `cargo test -p zirk-ir --test lowering -- --test-threads=1` — passed 169 lowering tests, including `ServiceStart`/`ServiceReady`.
- `cargo test -p zirk-runtime --lib services::tests -- --test-threads=1` — passed 17 service supervisor tests.
- `cargo test -p zirk-runtime --test staticlib service_abi_registers_publishes_readiness_and_joins_shutdown -- --exact --test-threads=1` — passed 1 ABI lifecycle test.
- `cargo test -p zirk-codegen-llvm --test emission -- --test-threads=1` — passed 62 LLVM emission tests.
- `cargo test -p zirk-cli --test end_to_end service_source_callbacks_register_ready_and_shutdown_cleanly -- --exact --test-threads=1` — passed; its four source fixtures cover lambda and named callbacks, readiness lifecycle failure, and typed capacity handling.
- Browser preflight classifies `ServiceStart` and `ServiceReady` as native-only. `crates/zirk-browser/tests/corpus_parity.rs` lists all four service E2E fixtures as explicit capability exclusions, and the corpus parity test passed in the full workspace rerun.
- `cargo test -p zirk-runtime --lib collector::stw::tests -- --test-threads=1` — passed 7 tests, including panic/unwind recovery of the STW request gate and participant cleanup.
- `cargo test --workspace --quiet` — first run exposed browser capability exclusions missing for the new service IR. After adding the interpreter classification and four explicit fixture exclusions, a parallel run passed the complete CLI E2E suite (71/71), but four `zirk-runtime` unit tests failed under test-thread contention while mutating process-global signal/GC/executor state. The same full suite passed when serialized, as recorded below; the parallel-only failures are retained as a concurrency test-isolation limitation.
- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,329 tests, 15 ignored, across 74 suites** after service integration, browser capability exclusions, and abstract mutex CFG ownership markers.
- `cargo test -p zirk-ir --test verification -- --test-threads=1` — passed **55 tests**, including branch join/backedge consistency, duplicate/unmatched acquire/release, held-lock exits, and rejection of a potentially suspending mutex acquire while a BorrowedView is open.
- `cargo check --workspace` — passed after adding the abstract mutex markers; source-to-LLVM compilation explicitly fails with `E0105` if those verifier-only markers are present in a codegen module.
- `cargo check -p zirk-runtime --tests --target x86_64-pc-windows-msvc` and `cargo clippy -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc -- -D warnings` — passed. Windows linking and actual test execution remain unverified on this macOS host.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after the CFG marker integration.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after service integration and browser exclusion updates.

## BorrowedView transitive direct-call effect verification (2026-10-13)

- The IR effect analysis now builds reverse direct-call edges and propagates boundary effects with a worklist. This replaces repeated whole-module rescans and handles recursive call graphs. Unknown direct targets remain effectful. Contract, virtual, closure, callable, scheduling, waits, host I/O, channel operations, GC allocations, safepoints, and mutex acquisition fail closed while a BorrowedView is open.
- `cargo test -p zirk-ir --test verification -- --test-threads=1` — passed 58/58, including a direct-call chain accepted when every body is synchronous, transitive safepoint rejection, direct host-I/O and safepoint rejection, and mutex acquisition rejection while a BorrowedView is open.
- `cargo test -p zirk-ir --test lowering -- --test-threads=1` — passed 169/169.
- This proves the current IR verifier behavior only. It does not establish the sema-to-IR lifetime path, Mutex source API, GC payload descriptor, runtime unlock cleanup, or Windows execution.

## Managed worker channel exclusion (2026-10-13)

- `Thread.run` effect summaries now track channel use independently of suspension. Construction and every channel method seed the effect; direct-call summaries propagate it transitively. The worker gate rejects the callback with `E0484`, including nonblocking `try_send`/`try_receive` and `close`, as required by the raw-thread contract.
- `cargo test -p zirk-sema --test typing -- --test-threads=1` — passed 631/631; includes direct nonblocking channel use in a captured lambda and channel use through a direct helper.
- `cargo test -p zirk-cli --test end_to_end thread_run_checks_effects_and_accepts_complete_callbacks -- --exact --test-threads=1` — passed; includes the new invalid source fixture `thread_run_try_channel.zrk`.
- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,333 tests, 15 ignored, across 74 suites** after the transitive BorrowedView boundary analysis and Thread.run channel exclusion.
- `cargo test -p zirk-codegen-llvm --lib borrowed_view_ir_cannot_be_silently_compiled_as_an_object -- --test-threads=1` — passed 1/1. The normal object-compilation entry point now rejects BorrowedView markers with `E0105`, just like unlowered Mutex acquire/release markers; a verifier-only token cannot be mistaken for a real traced payload view.
- `cargo clippy -p zirk-codegen-llvm --all-targets -- -D warnings` — passed after the codegen preflight change.

## Internal GC-managed Mutex payload foundation (2026-10-13)

- Added a private per-object allocation containing collector header, Rust mutex metadata, and inline payload. Payload offsets are aligned against the allocation base; nested managed-reference offsets are validated and traced by the collector. References in the by-value payload are temporarily rooted across `zirk_rt_alloc`, and sweep drops both payload and Rust metadata once. Payload alignment beyond the collector's current supported bound fails closed.
- `cargo test -p zirk-runtime --lib gc_mutex::tests -- --test-threads=1` — passed 3/3: alignment after the GC header, nested-reference survival under forced collection, and exactly-once sweep destruction.
- `cargo clippy -p zirk-runtime --all-targets -- -D warnings` — passed after integrating the allocation and collector hooks.
- This is runtime storage groundwork only. No lock acquire/release ABI, source type, BorrowedView payload projection, cancellation path, or compiler cleanup lowering is implemented.

## Mutex cleanup CFG and Windows execution audit (2026-10-14)

- Added `mutex_cfg_verifier_checks_cancel_and_throw_cleanup_edges` to verify that a held abstract mutex is rejected at a cancellation exit and at an uncaught-throw return, while an explicit release before throw propagation is accepted. This proves balance of verifier markers only; it does not prove a runtime unlock.
- `cargo test -p zirk-ir --test verification -- --test-threads=1` — passed 59/59 after the new regression.
- `cargo test --workspace --quiet -- --test-threads=1` — completed successfully on the current checkout with no failing suite. The full run began before the new verifier regression was added; the focused 59-test run above includes it.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed after the regression and task record were updated.
- A read-only Windows feasibility audit confirmed the current host is macOS ARM64 with no Windows VM, Wine, PowerShell, or native Windows runner. Cross-target `cargo check`/Clippy and `lld-link` do not establish Windows execution. Existing GitHub Windows runs are for a different commit (`ba301ba7`), not this dirty checkout; executing the current changes requires a Windows runner to receive this state. Task 5.6 therefore remains open.
- Static review of the verifier found no concrete CFG lock-balance defect: reachable `Jump`/`Branch` edges, joins, loops, and terminal exits are handled. The remaining 8.3 gap is the missing source lowering, real lock ABI, and cleanup code that emits unlock operations on every exit.
- The runtime `CooperativeMutex` now exposes internal `acquire()`/`release()` methods for a future generated-code boundary; `with()` continues to use an RAII guard. Release checks executor affinity and ownership. The GC mutex metadata now embeds this cooperative primitive and exposes its inline payload pointer only through an unsafe, paired acquire/release API.
- Added `cooperative_gc_mutex_exposes_payload_only_during_owned_lock`, which allocates the GC mutex inside an executor task, mutates its inline payload under the lock, releases, and reacquires to confirm the stored value. The runtime integration test harness includes `sync_mutex.rs` so the collector's isolated compilation harness remains valid.
- `cargo test -p zirk-runtime --lib gc_mutex::tests -- --test-threads=1` — passed 5/5, including a forged aligned non-GC pointer rejected before its header is read.
- `cargo test -p zirk-runtime --lib sync_mutex::tests -- --test-threads=1` — passed 12, 1 ignored.
- `cargo test -p zirk-runtime --test optimization_bench -- --test-threads=1` — passed 299, 3 ignored, confirming the source-module test harness compiles and runs the added runtime checks.
- `cargo test -p zirk-runtime --lib -- --test-threads=1` — passed 364, 3 ignored after adding exact heap-membership and sentinel checks before resolving Mutex handles. `cargo check -p zirk-runtime --tests` and `cargo clippy -p zirk-runtime --all-targets -- -D warnings` passed.
- After that runtime change, `cargo check -p zirk-runtime --tests --target x86_64-pc-windows-msvc` and `cargo clippy -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc -- -D warnings` also passed. This is Windows-target compile/lint evidence only.
- This establishes runtime storage and lock ownership primitives only. There is still no exported mutex ABI, managed handle in the lock IR, source `Mutex.with`, or compiler-emitted release cleanup; the unsafe payload pointer is not exposed to source code.
- After the cooperative-lock integration, `cargo fmt --all -- --check`, `git diff --check`, `openspec validate concurrency-completion --strict`, and `cargo test --workspace --quiet -- --test-threads=1` all completed successfully. The workspace command exited 0 with no failing suite. The subsequently added heap-membership validation passed the complete runtime library suite; rerun the workspace suite after the current IR update is integrated.


## Typed Mutex handles in verified IR (2026-10-14)

- Added `IrType::Mutex(payload_type_id)` and a validated module payload-type table. Mutex handle slots are GC-managed roots; borrowed views encode both their lexical region and protected payload type.
- Mutex acquire/release markers refer to stable rooted slots rather than block-local SSA values. The reachable-CFG verifier binds each lock identity to its handle slot, rejects mismatched release slots and mutation of the held handle slot, and retains existing duplicate acquire, unmatched release, join/backedge, cancellation, throw, and exit checks.
- Added verifier coverage for mismatched handles and handle-slot replacement while held; the verifier suite passes 61/61.
- `cargo test -p zirk-ir -p zirk-codegen-llvm -p zirk-interpreter -- --test-threads=1` — passed across all 13 suites, including 61 verifier tests, 169 lowering tests, 62 LLVM emission tests, 2 managed-atomic tests, and 59 interpreter tests.
- `cargo clippy -p zirk-ir -p zirk-codegen-llvm -p zirk-interpreter --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.
- This is a verified-IR representation only. No source Mutex construction/`with` lowering, runtime ABI emission, payload field projection, unlock cleanup generation, or execution from `Thread.run` is enabled.


## Mixed executor and managed-worker mutex waits (2026-10-14)

- `CooperativeMutex` now has distinct participant identities for executor tasks and registered managed workers. Tasks park through the scheduler; only registered workers can block on the condition variable. FIFO handoff covers both, and task cancellation removes queued or reserved ownership before passing it onward.
- A registered worker enters collector `InNative` while it blocks on the condition variable and resumes before it can access the protected payload. A regression holds the mutex in an executor task, observes the worker in the native wait, forces stop-the-world collection, then releases and confirms panic-safe worker acquisition/release.
- `cargo test -p zirk-runtime --lib sync_mutex::tests -- --test-threads=1` — passed 14, 1 ignored.
- `cargo test -p zirk-runtime --lib gc_mutex::tests -- --test-threads=1` — passed 5/5. The alignment regression now checks only required padding: payload alignment is enforced even when the body end already lies on an aligned offset.
- `cargo clippy -p zirk-runtime --all-targets -- -D warnings` — passed.
- No source Mutex API or generated-code cleanup is enabled; this closes only the runtime mixed-participant foundation and its GC wait boundary.


## Integrated current-checkout validation (2026-10-14)

- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,367 tests, 16 ignored, across 74 suites** after the typed Mutex IR, mixed task/worker mutex, collector native-wait transition, and GC payload-alignment assertion updates.
- `cargo test -p zirk-runtime --lib -- --test-threads=1` — passed **366 tests, 3 ignored**.
- `cargo check -p zirk-runtime --tests --target x86_64-pc-windows-msvc` and `cargo clippy -p zirk-runtime --all-targets --target x86_64-pc-windows-msvc -- -D warnings` — passed. These commands compile/lint for Windows; the code did not execute on Windows.
- `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.

## Mutex source typing, borrowed-lock verification, and fail-closed LLVM emission (2026-10-14)

- Sema recognizes `Mutex<T>` and checks `Mutex.with` closure shape, payload type, borrowed provenance/escape rules, and complete transitive no-suspend/no-block/no-spawn effects. The constructor and `.with` call remain `NOT_LOWERED`; this does not enable source execution.
- `Mutex<T>` handles participate in reference alias checks; they remain non-Transfer/non-Share under the conservative ownership default until a cross-task sharing contract is integrated.
- The parser accepts the reserved `with` keyword after `.`/`?.`, and sema typing regressions now exercise `.with` directly instead of rewriting an AST placeholder.
- IR lowering interns the protected payload layout and represents source `Mutex<T>` variables as GC-rooted typed handles. A lowering regression verifies the handle slot is a root and the payload table contains `Int32`.
- The IR verifier requires `BorrowViewStart` to use a handle loaded from the slot whose lock is held, and rejects release while the associated borrowed view remains active. Regressions cover unlocked access and release-before-view-end.
- Raw LLVM `emit` now inserts `llvm.trap` for verifier-only Mutex/BorrowView markers. The ordinary object-compilation entry point continues to reject those markers with `E0105`.
- `cargo test -p zirk-parser reserved_with_keyword_is_allowed_after_member_access_operator` — passed.
- `cargo test -p zirk-sema mutex_ -- --test-threads=1` — passed six focused Mutex typing tests, including reference aliasing; `cargo test -p zirk-sema mutex_with_marks_scalar_payload_view_mutable -- --exact` passed the scalar writable-view regression. The prior full `cargo test -p zirk-sema` run passed 674 tests before the two latest regressions were added.
- `cargo test -p zirk-ir --test verification -- --test-threads=1` — passed 64/64 after adding the lock/view coupling checks.
- `cargo test -p zirk-ir --test lowering mutex_type_lowers_to_a_rooted_handle_and_interned_payload_type -- --exact` — passed.
- `cargo test -p zirk-codegen-llvm borrowed_view_raw_llvm_emission_fails_closed_until_payload_lowering_exists -- --nocapture` — passed.
- `cargo test -p zirk-cli --test end_to_end -- --test-threads=1` — passed 72/72 after replacing the obsolete `mutex_pending_type` fixture with `mutex_operations_not_lowered` and separating Mutex's known type from the other Phase 5 pending-type fixtures. `Mutex<T>` is recognized; constructor and `.with` report `E0423` until lowering exists.
- `cargo test -p zirk-sema -- --test-threads=1` — passed 678 tests after adding the Mutex reference-alias, writable scalar-view, and direct/transitive nested-acquisition effect regressions.
- `cargo test -p zirk-sema nested_mutex_with_is_a_suspend_and_block_effect_under_the_outer_guard -- --exact` — passed; a nested acquisition is included in the outer callback's effect summary and rejected under the active view.
- `cargo test -p zirk-ir --test verification --test lowering -- --test-threads=1` — passed 234 tests after lock/view coupling and Mutex payload-type lowering.
- `cargo test -p zirk-codegen-llvm --test emission -- --test-threads=1` — passed 62 tests; raw LLVM emission traps for all four verifier-only Mutex/BorrowView markers.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed after these source, parser, IR, test, and documentation changes.
- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,377 tests, 16 ignored, across 74 suites** after updating the obsolete Mutex Phase 5 gate fixture. This run preceded the last nested-acquisition sema regression; the final sema suite and workspace Clippy were rerun afterward.
- `cargo clippy --workspace --all-targets -- -D warnings` — passed again after recording nested Mutex acquisition as a transitive suspension/block effect.
- These focused checks establish parsing, sema rejection rules, IR metadata/verification, and fail-closed emission separately. They do not establish runtime mutex ABI, payload field/aggregate projections, or unlock cleanup on generated return/exception/cancellation paths. Re-run full workspace validation after the remaining compiler integration work.

## 2026-10-14 final Mutex prerequisite and Windows execution audit

- `cargo test --workspace --quiet -- --test-threads=1` — passed **2,379 tests, 16 ignored, across 74 suites** (162.86 s), including the latest nested `Mutex.with` suspension/block effect regressions.
- `cargo test -p zirk-runtime --lib sync_mutex::tests -- --test-threads=1` — passed 14 tests; 1 ignored.
- `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` — passed.
- Read-only independent audits reconfirmed that 6.7, 8.1, and 8.3 remain partial: source construction/`.with` are gated; Mutex IR markers do not execute; and generated source cleanup does not release the lock on return, exception, or cancellation edges. Keep all three open until the source-to-runtime path and its exit-path tests exist.
- Attempted to use the local `Windows 11` VirtualBox guest for 5.6. It boots as Windows 11 ARM64 with Guest Additions 7.2.8, but the only visible account (`vboxuser`) is blocked by an expired-password change screen and no guest session is active. No password was changed and no Windows execution is claimed. Existing Windows cross-target compile/Clippy evidence remains as recorded above; task 5.6 stays open.

## Mutex runtime ABI groundwork (2026-10-14)

- Added an exported C ABI for GC-managed inline Mutex payload allocation, acquire, and release. Allocation validates native layout, caps payload/reference metadata, checks every traced pointer offset, roots staged managed references across a possible collection, and reports status without unwinding across C. Zero-sized payloads use a one-byte backing allocation while retaining logical size zero.
- Runtime unit coverage verifies malformed offsets, reentrancy status, nested managed-reference tracing across forced collection, and zero-sized payload acquisition/release. `cargo test -p zirk-runtime --lib gc_mutex::tests -- --test-threads=1` passed 8 tests.
- Added C/Rust signature assertions and symbol export checks for the three ABI functions. Agent validation reported `cargo test -p zirk-runtime --test staticlib` (9 passed), `ZIRK_ABI_TARGETS=aarch64-apple-darwin scripts/validate-concurrency-abi-signatures.sh` (31 signatures matched), and `python3 scripts/test_concurrency_abi_signatures.py` (6 passed).
- LLVM now declares the three native 64-bit ABI signatures. `cargo test -p zirk-codegen-llvm gc_mutex_runtime_declarations_match_the_native_abi_shapes -- --test-threads=1` passed.
- These checks establish an ABI foundation only: generated code does not yet call it, payload field/aggregate projections are not lowered, and generated exits do not release the lock. Tasks 6.7, 8.1, and 8.3 remain open; source Mutex continues to fail closed.
- Final cross-workspace gate after the ABI/declaration changes: `cargo test --workspace --quiet -- --test-threads=1` passed **2,387 tests, 16 ignored, across 75 suites** (156.61 s). `cargo clippy -p zirk-runtime -p zirk-codegen-llvm --all-targets -- -D warnings`, `cargo fmt --all -- --check`, `git diff --check`, and `openspec validate concurrency-completion --strict` passed.

## Synchronous Mutex callback cleanup ABI (2026-10-14)

- Added `zirk_rt_gc_mutex_with`, which validates the managed handle, acquires the cooperative mutex, invokes a non-null `C-unwind` callback with the rooted payload pointer, and releases through an RAII guard on return or Rust unwind. A release ownership violation is treated as fatal; callback unwind returns status 4 only after the guard releases.
- `mutex_with_releases_the_lock_when_callback_unwinds` mutates the payload, injects unwind, checks status 4, then reacquires and observes the mutation. `cargo test -p zirk-runtime --lib gc_mutex::tests -- --test-threads=1` passed 9/9.
- Added the C consumer callback type assertion, exported-symbol/prototype checks, and LLVM declaration. `cargo build -p zirk-runtime` refreshed the static archive; `cargo test -p zirk-runtime --test staticlib -- --test-threads=1` passed 9/9. The ABI checker matched 32 Rust/C symbols for `aarch64-apple-darwin`; its Python suite passed 6/6. The focused LLVM ABI declaration test passed 1/1.
- This makes runtime cleanup exception-safe for the synchronous native callback boundary. Generated code still does not invoke the callback ABI, source-language borrowed parameters are still represented as copied values in lifted lambda bodies, and source exception/cancellation edges remain unintegrated. Tasks 6.7/8.1/8.3 remain open.
