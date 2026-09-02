## Why

`phase-4e-unsafe-pointer-extern` (merged) delivered `unsafe fn`/`unsafe {}`/`commit {}` parsing and context-checking, `Pointer<T>` construction/read/write/offset/cast, and `extern "C" fn` — but explicitly deferred its own design D5/D6 (the transactional journal/rollback contract itself). Today `unsafe {}` does not record writes and `commit {}` does not durably publish anything, so nothing rolls back on a controlled failure — the single biggest normative gap that change's own "Explicitly out of scope" and `docs/handbook/13-appendices/07-current-limitations.md` both flag. `MEMORY_AND_UNSAFE_SEMANTICS.md`'s own text (`unsafe {}` guards writes that "commit together or roll back on controlled pre-commit failure") is not yet true of the compiler.

The design for this was already written and left ready to implement (`openspec/changes/archive/2026-08-24-phase-4e-unsafe-pointer-extern/design.md` D5/D6), specifically flagged there as the natural second cut if the original change proved too large for one pass — which is exactly what happened.

## What Changes

- New `zirk-runtime` module for a per-`unsafe`-block undo log: `zirk_rt_journal_begin`, `zirk_rt_journal_record(journal, address, len)` (snapshots bytes before the write it guards), `zirk_rt_journal_commit` (discards the log without restoring), `zirk_rt_journal_rollback` (restores every recorded snapshot in reverse order, then discards).
- IR lowering wraps every `unsafe { ... }` block: `journal_begin` on entry; before every `Store`/`StoreField` inside the block whose target is a managed slot/field declared *outside* the block, a `journal_record` call; `journal_commit` on normal fall-through. A local declared inside the block itself is not journaled (nothing outside the block could observe rolling back a write to storage the block itself allocated).
- `commit { ... }` durably commits everything journaled by the enclosing `unsafe` block so far (`journal_commit`) before lowering its own body normally — publishing reversible writes before the irreversible native effect the commit boundary exists to guard.
- Rollback-on-exception: an exception becoming pending inside an `unsafe {}` block (a `throw`, a throwing call, or an implicit native safety check — detected the same way `lower_throws_check` already detects it after every call) triggers `journal_rollback` instead of `journal_commit` at the block's normal-exit check point, then lets propagation continue exactly as it already does elsewhere.
- Explicitly accepted, documented limit (unchanged from the original design's own risk note): a write performed by an `extern` call into memory the compiler cannot see (e.g. `memcpy` into a pointer target) is not journaled — `MEMORY_AND_UNSAFE_SEMANTICS.md` itself only promises rollback for Zirk-managed state and validated native ranges, routing unknown-effect native calls through `commit` (irreversible) instead, which is the normatively correct behavior, not a gap this change needs to close.

### Explicitly out of scope

- **Journaling writes through a `NativeSlice<T>`/`NativeSliceMut<T>` view** — that type doesn't exist yet (`phase-4e-native-slice`, parallel work); this change's journal only guards `Pointer<T>`/managed-slot writes already lowered today.
- **Journaling an `extern` call's own internal writes** — see "What Changes"; explicitly out of scope by the original design.
- **Nested `unsafe {}` blocks sharing one journal** — each `unsafe {}` gets its own journal scoped to itself; a nested block's rollback does not need to unwind the outer block's own journal beyond ordinary exception propagation already handling that via the outer block's own exit check.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-memory-safety`: the existing "Unsafe transaction" glossary concept and `MEMORY_AND_UNSAFE_SEMANTICS.md`'s own rollback text become normatively backed by an actual requirement (reaffirmed/added — checked against existing spec text during design).

## Impact

- Affected code: `crates/zirk-ir` (journal begin/record/commit/rollback IR instructions, wired into existing `unsafe`/`commit` lowering and the existing exception-pending check point), `crates/zirk-codegen-llvm` (lowering those instructions to `zirk-runtime` calls), `crates/zirk-runtime` (new journal module).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md` "transactional journal/rollback itself" line updated from "not implemented" to delivered. `../zirk-lang-site` sync required once this lands.
- No breaking changes: existing `unsafe {}`/`commit {}` programs continue to compile and run identically when no rollback path is triggered; this only adds behavior on a previously-undefined failure path.
