## Context

`fase-4e-unsafe-pointer-extern` (merged) built `unsafe fn`/`unsafe {}`/`commit {}` context-checking and `Pointer<T>`'s operation set, but its own design explicitly deferred D5/D6 (the journal itself) as the natural second cut when the original change proved too large for one pass. That design's D5/D6 text is reproduced and carried forward here verbatim (already reviewed and accepted as the shape to build, not re-derived) — this document exists to confirm nothing about the substrate has changed since (`fase-4e-colector-mark-sweep`/`fase-4e-weak` both merged since; neither touches `unsafe`/`commit` lowering) and to record any refinement needed now that implementation is actually starting.

## Goals / Non-Goals

**Goals:**
- Every managed-slot/field write inside an `unsafe { ... }` block, targeting storage declared outside that block, is journaled before it executes.
- Normal fall-through exit durably commits the journal (discards without restoring).
- `commit { ... }` durably commits the enclosing `unsafe` block's journal so far, before running its own body — publishing reversible writes before the irreversible effect the commit boundary exists to guard.
- An exception becoming pending inside the block (via `throw`, a throwing call, or an implicit native safety check) rolls back every journaled write in reverse order before propagation continues.
- A write to storage allocated *inside* the block itself is not journaled (nothing outside the block could observe rolling it back).

**Non-Goals:** (see proposal's "Explicitly out of scope" — `NativeSlice<T>` writes, `extern` call internal writes, cross-nested-block journal sharing.)

## Decisions

### D1 (carried forward as D5 in the original design): The journal is a single per-`unsafe`-block undo log of `(address, byte_length, old_bytes)` records, materialized in `zirk-runtime`

New `zirk-runtime` module (`journal.rs`): `zirk_rt_journal_begin() -> *mut Journal`, `zirk_rt_journal_record(journal, address, len)` (snapshots `len` bytes at `address` into the journal *before* the write it guards executes), `zirk_rt_journal_commit(journal)` (discards the log without restoring — the durable case, used both at normal block exit and at `commit {}`), `zirk_rt_journal_rollback(journal)` (restores every recorded snapshot in reverse order, then discards the log).

IR lowering for `unsafe { ... }` wraps the block: `journal_begin` on entry; before every `Store`/`StoreField` reachable inside the block whose target is a managed slot/field declared outside the block, a `journal_record` call; on the block's normal fall-through path, `journal_commit`. A local declared inside the block is tracked via the existing scope/slot-declaration bookkeeping the checker already has (a slot's declaring block is already known for ordinary scoping purposes) — the "is this slot's declaration lexically inside the current `unsafe` block" check reuses that, not new tracking.

### D2 (carried forward as D6): `commit {}` durably commits at its own entry; rollback-on-exception reuses the existing pending-exception mechanism

`commit { ... }` calls `journal_commit` on the *enclosing* `unsafe` block's journal at commit's own entry (before lowering commit's own body normally) — publishing everything journaled so far. An exception becoming pending inside an `unsafe {}` block is detected the same way `lower_throws_check` already detects it after every call (`fase-4b`'s D1-D3, made unconditional by `native-runtime-errors-catcheable`'s D11): this change adds one more check point — immediately before the unsafe block's own normal-exit `journal_commit`, if the pending-exception slot is set, call `journal_rollback` instead, and let propagation continue exactly as it already does elsewhere (closing newly-acquired resources first, per `MEMORY_AND_UNSAFE_SEMANTICS.md` §10's ordering, reusing whatever resource-cleanup-on-unwind path `fase-4c-recursos` already built).

### D3: Journal records are raw byte snapshots taken via the same load path the write itself would use, sized from the slot/field's static type — no dynamic type tag needed

Since every journaled write target is a statically-typed managed slot or field (D1's own scope restriction), `journal_record`'s `len` is always known at the IR-lowering call site from the target's IR type — no runtime size discovery, no dynamic dispatch. This mirrors why `fase-4e-colector-mark-sweep` fixed `GC_ALIGN` at a compile-time constant rather than storing per-allocation alignment: the information needed is already statically available at the call site, so nothing new needs to flow through the runtime boundary.

### D4: One journal per `unsafe` block, not a global/thread-local stack of journals

Each `unsafe { ... }` block's `journal_begin`/`journal_commit`(-or-`rollback`) pair is fully self-contained — the `*mut Journal` handle is a local SSA value in the lowered IR (spilled to a slot like any pointer-typed local would be, no shadow-stack rooting needed since it is a `zirk-runtime`-internal allocation outside the collector's object model, same category as `Weak<T>`'s WeakCell context or `fase-4e-clone`'s memoization table). Nested `unsafe` blocks simply nest their own independent begin/commit-or-rollback pairs; an inner block's rollback does not need special coordination with an outer block's own journal beyond ordinary exception propagation (the inner block's own rollback already runs before the exception reaches the outer block's own check point, which then makes its own commit/rollback decision independently based on whether the exception is still pending).

## Risks / Trade-offs

- **[Risk] Journaling every managed write inside every `unsafe` block adds a runtime call before each such write, even on the hot path where nothing ever fails.** → Accepted: this is the literal cost of the transactional guarantee `MEMORY_AND_UNSAFE_SEMANTICS.md` already promises; `unsafe` code is expected to be a narrow, deliberately-entered surface, not general hot-path code. No mitigation attempted in this change; a future optimization (e.g., only journaling once per distinct address within a block) is out of scope here.
- **[Risk] `journal_record`'s byte-snapshot approach cannot detect two overlapping writes to the same address needing only one snapshot** — each `Store`/`StoreField` gets its own record even if it targets the same address as an earlier one in the same block. → Accepted: correctness is unaffected (rollback in reverse order restores correctly regardless of duplication), only a minor memory/time cost; deduplication is a future optimization, not required for this change's correctness goal.
- **[Trade-off] The "declared outside the block" rule (D1) requires reusing the checker's existing slot-declaration-scope tracking rather than adding a new one** — correct and cheap, but means this change is coupled to that tracking already being accurate for every slot kind (including synthetic spill slots from `fase-4e-colector-mark-sweep`'s D4). Documented here since a synthetic spill slot's "declaring block" must be understood correctly (as the block containing the instruction whose result it spills, not the `unsafe` block that happens to contain that instruction) for this rule to journal the right things.
