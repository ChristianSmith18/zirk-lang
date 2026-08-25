## 1. Runtime (zirk-runtime)

- [ ] 1.1 New module `journal.rs`: `zirk_rt_journal_begin() -> *mut Journal` (allocates an undo log, not collector-tracked — same category as the frame stack), `zirk_rt_journal_record(journal, address: *mut u8, len: usize)` (snapshots `len` bytes at `address` into the log before the write it guards executes), `zirk_rt_journal_commit(journal)` (discards the log without restoring, frees the journal itself), `zirk_rt_journal_rollback(journal)` (restores every recorded snapshot in reverse order, then discards/frees).
- [ ] 1.2 Runtime unit tests: record then commit leaves memory unchanged and frees the log; record then rollback restores the original bytes; multiple records to different addresses roll back all of them in reverse order; multiple records to the *same* address roll back correctly to the original pre-block value (not an intermediate one) given reverse-order restoration.

## 2. IR (zirk-ir)

- [ ] 2.1 Add journal IR instructions: `JournalBegin`, `JournalRecord(address, len)`, `JournalCommit`, `JournalRollback`.
- [ ] 2.2 Lower `unsafe { ... }`: `JournalBegin` on entry. Before every `Store`/`StoreField` inside the block whose target is a managed slot/field declared *outside* the block (reuse the existing slot-declaration-scope tracking the checker already has for ordinary scoping — verify it correctly reports a synthetic GC-spill slot's declaring block as the block containing the instruction whose result it spills, per design D1's own note, not the `unsafe` block that happens to contain that instruction), emit `JournalRecord` before the store. A slot declared inside the block itself is not journaled. On the block's normal fall-through exit, emit `JournalCommit`.
- [ ] 2.3 Lower `commit { ... }`: at commit's own entry, emit `JournalCommit` against the *enclosing* `unsafe` block's journal (durably publishing everything journaled so far), before lowering commit's own body normally.
- [ ] 2.4 Rollback-on-exception: reuse the existing pending-exception check point (`lower_throws_check`'s mechanism, already run after every call per `fase-4b`/`native-runtime-errors-catcheable`'s D11) — add one more check immediately before the unsafe block's own normal-exit `JournalCommit`: if the pending-exception slot is set, emit `JournalRollback` instead, then let propagation continue exactly as it already does (closing newly-acquired resources first, per `MEMORY_AND_UNSAFE_SEMANTICS.md` §10's ordering — reuse `fase-4c-recursos`'s existing resource-cleanup-on-unwind path, don't build a second one).
- [ ] 2.5 IR-level tests: an `unsafe {}` block with a managed write lowers to `JournalBegin` → `JournalRecord` → (write) → `JournalCommit` on the normal path; a `commit {}` inside an `unsafe {}` block emits `JournalCommit` at commit's own entry against the outer journal; a write to a slot declared inside the `unsafe` block itself does NOT get a `JournalRecord`; the exception-pending check point correctly branches to `JournalRollback` instead of `JournalCommit`.

## 3. Codegen (zirk-codegen-llvm)

- [ ] 3.1 Emit `JournalBegin`/`JournalRecord`/`JournalCommit`/`JournalRollback` as calls into the new `zirk-runtime` journal functions (task 1.1). The `*mut Journal` handle is an ordinary local SSA pointer value, not a managed reference — confirm it does NOT get spilled to a GC root slot (it's a `zirk-runtime`-internal allocation outside the collector's object model, same category as `Weak<T>`'s WeakCell context).

## 4. Cross-cutting correctness tests (do not skip — this is a soundness-critical feature, same standard as the collector)

- [ ] 4.1 End-to-end `.zrk` fixture: an `unsafe {}` block mutates managed state, then completes normally — the write is durably visible after the block.
- [ ] 4.2 End-to-end `.zrk` fixture: an `unsafe {}` block mutates managed state, then propagates a validation `Error` (matching the spec's own "Validation fails after managed writes" scenario) — confirm the managed state is restored to its pre-block value before the error escapes. This is the single most important test in this change.
- [ ] 4.3 End-to-end `.zrk` fixture: an `unsafe {}` block mutates managed state, then enters `commit {}` — confirm the write is durably committed (survives even if something after the commit boundary later fails, since commit publishes before the irreversible effect per the spec's "Unsafe transaction isolation" requirement).
- [ ] 4.4 End-to-end `.zrk` fixture: an `unsafe {}` block mutates two different managed locations, then throws — confirm BOTH are restored (tests reverse-order multi-record rollback for real, not just in the runtime unit test).
- [ ] 4.5 End-to-end `.zrk` fixture: an `unsafe {}` block declares a local, mutates it, then throws — confirm the throw still propagates correctly and nothing crashes from attempting to roll back a write that was correctly never journaled (the "declared inside the block" exemption, exercised for real).
- [ ] 4.6 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` (multiple times, watching for flakiness); `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 5. Documentation and status sync

- [ ] 5.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md`'s memory rows — "the transactional journal/rollback itself" moves from "not implemented" to delivered.
- [ ] 5.2 Check `docs/handbook`/`docs/MEMORY_AND_UNSAFE_SEMANTICS.md` for any existing `unsafe`/`commit` teaching material with a stale "does not roll back yet" caveat — reconcile.
- [ ] 5.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 5.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 6. OpenSpec close-out

- [ ] 6.1 Run `openspec validate fase-4e-unsafe-journal` before archiving.
- [ ] 6.2 Archive the change once implementation, tests, and documentation sync are complete.
