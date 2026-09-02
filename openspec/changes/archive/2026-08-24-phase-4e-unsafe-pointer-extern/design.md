## Context

Zirk's pipeline is lexer → parser (AST) → checker (`zirk-sema`) → IR lowering (`zirk-ir`) → LLVM codegen (`zirk-codegen-llvm`) → link (`zirk-native-codegen`'s own linking step) + `zirk-runtime` (a static library the executable links). Locals already live in named IR "slots" (`InstKind::Load(SlotId)`/`Store(SlotId, Operand)`), which in codegen become LLVM `alloca`s — every local already has a stable address, which is what makes `Pointer.from(local)` tractable without inventing a new storage model. `docs/decisions/ADR-015-declaracion-extern.md` (this session) already decided the `extern "C" fn` syntax and scope; this document covers the rest of the slice the proposal describes.

## Goals / Non-Goals

**Goals:**
- `unsafe fn`/`unsafe {}`/`commit {}` parse, and the checker enforces which operations require which context.
- `Pointer<T>` (narrow ABI-safe `T`) is a real type: construct via `Pointer.from(place)`, `.is_null`/`.read()`/`.write()`/`.offset()`/`.offset_bytes()`/`.cast<U>()`.
- A `Pointer<T>` obtained from `Pointer.from` cannot escape (return, field, closure capture) — sound, conservative, no per-value provenance tracking.
- `unsafe {}` journals managed writes made through it and rolls them back on a controlled failure before that block's own completion or `commit`; `commit {}` durably commits the journal so far, then runs irreversible effects.
- `extern "C" fn` declarations parse, type-check against the ADR-015 type surface, lower to genuine external symbol declarations, and link against whatever the system linker already resolves.

**Non-Goals:** (see proposal's own "Explicitly out of scope" — not repeated here.)

## Decisions

### D1: `Pointer<T>` is its own `Base` variant, not routed through user generics

`Base::Pointer(Box<Type>)` (or an interned equivalent, matching how other composite types in `zirk-sema/src/types.rs` are represented) rather than reusing the general user-generic-class instantiation machinery (`Base::Instance`/`generic_instances`). `Pointer<T>` is compiler-built-in with a fixed, closed operation set — it does not need constructor resolution, method-table dispatch, or specialization the way a user generic class does. Keeping it a dedicated variant is simpler and avoids teaching the generic-instantiation path about a type it was never designed for.

### D2: The type surface is a closed allow-list, checked once at `Pointer<T>` construction (any position: annotation, `Pointer.from`, `.cast<U>()`)

A single function, `fn is_ffi_safe(ty: Type) -> bool`, recursively allows `Void` (only as the pointee of nothing — `Pointer<Void>` itself is not constructed by this slice; reserved for a future "opaque pointer" case, not built here), `Boolean`, every fixed-width `Int`/`UInt`, `Float32`/`Float64`, and `Base::Pointer(inner)` where `inner` is itself FFI-safe. Anything else (String, class, record, enum, contract, `Fn(...) => R`) is rejected wherever a `Pointer<T>` is named, with a diagnostic naming the disallowed `T`. `extern "C" fn` signatures reuse the same predicate for every parameter and the return type (return additionally allows `Void`).

### D3: Unsafe/commit context is a small stack on the checker, mirroring `loop_depth`

Two counters (or a small enum stack, if extern-call-specific messaging needs to distinguish "missing unsafe" from "missing commit"): `unsafe_depth: u32` and `commit_depth: u32`, incremented/decremented around `check_block` for `unsafe {}`/`commit {}` respectively. A pointer operation requiring `unsafe` checks `unsafe_depth > 0`; an `extern` call site checks both `unsafe_depth > 0` and `commit_depth > 0`. `commit {}` itself requires `unsafe_depth > 0` at the point it opens (checked before incrementing `commit_depth`), matching `check_loop`'s existing pattern of validating context before entering a nested checked region.

### D4: The escape rule is a blanket ban, not per-value provenance tracking

Any expression of static type `Pointer<T>` (regardless of how it was produced — `Pointer.from`, `.offset()`, a parameter, whatever) is rejected as a `return` value, a field-write source, or a value captured by a closure. This is deliberately more conservative than a real borrow checker (it would also reject a hypothetically-safe case, like returning a pointer whose owner the caller already keeps alive some other way) but it is sound, requires no new lifetime-tracking machinery, and matches the spirit of the *already-normative* `NativeSlice<T>` escape scenario in `zirk-memory-safety` ("Native view escapes its borrow") — this change gives `Pointer<T>` the same treatment before `NativeSlice<T>` itself is built. A parameter of type `Pointer<T>` is exempt from the "cannot be returned" rule only insofar as *reading through it* is fine; passing the parameter's own value onward as a return value is still rejected, uniformly.

Alternative considered: track whether a specific `Pointer<T>` value's static provenance is "local" (from `Pointer.from` on something in the current frame) versus "external" (a parameter, or derived from one) and only reject the local case. Rejected for this slice — it requires flowing a taint bit through every expression that can produce or forward a `Pointer<T>` value (arithmetic, casts, conditional expressions, …), which is real dataflow analysis this change does not need yet: rejecting all returns/field-writes/captures of any `Pointer<T>` is strictly safe and dramatically simpler, and can be relaxed later without breaking any program that compiles under the stricter rule today.

### D5: The journal is a single per-`unsafe`-block undo log of `(address, byte_length, old_bytes)` records, materialized in `zirk-runtime`

New `zirk-runtime` module (`unsafe_journal.rs` or similar): `zirk_rt_journal_begin() -> *mut Journal`, `zirk_rt_journal_record(journal, address, len)` (snapshots `len` bytes at `address` into the journal *before* the write it guards executes), `zirk_rt_journal_commit(journal)` (discards the log without restoring — the "durable" case, used both at normal block exit and at `commit {}`), `zirk_rt_journal_rollback(journal)` (restores every recorded snapshot in reverse order, then discards the log). IR lowering for `unsafe { ... }` wraps the block: call `journal_begin` on entry; before every `Store`/`StoreField` reachable inside the block whose target is a managed slot/field (not a fresh local declared *inside* the block itself — nothing outside the block could observe rolling back a write to storage the block itself allocated, so those are not journaled, keeping the log to genuinely "before this block" state); on the block's normal fall-through path, call `journal_commit`; `commit { ... }` calls `journal_commit` at its own entry (durably committing everything journaled by the enclosing unsafe block so far) before lowering its own body normally.

Alternative considered: copy-on-write page-level snapshotting. Rejected — `MEMORY_AND_UNSAFE_SEMANTICS.md` §10 itself says the runtime "must not clone the entire reachable heap merely to enter a block", and page-level COW needs OS-level `mprotect`/fault-handling machinery this change has no reason to build for a first slice whose writes are individual scalar fields/pointer targets, not bulk memory.

### D6: Rollback-on-exception reuses the existing pending-exception mechanism (`fase-4b`'s D1-D3, made unconditional by `native-runtime-errors-catcheable`'s D11)

An exception becoming pending inside an `unsafe {}` block (from a `throw`, a call that throws, or an implicit native safety check) is detected the same way `lower_throws_check` already detects it after every call — this change adds one more check point: immediately before the unsafe block's own normal-exit `journal_commit`, if the pending-exception slot is set, call `journal_rollback` instead and let propagation continue exactly as it already does elsewhere (closing newly-acquired resources first, per `MEMORY_AND_UNSAFE_SEMANTICS.md` §10's own ordering, reusing whatever resource-cleanup-on-unwind path `fase-4c-recursos` already built rather than inventing a second one).

### D7: `extern "C" fn` lowers to an ordinary external function declaration; the call site lowers to an ordinary `Call`, gated by the checker's D3 rule

No new IR instruction kind for calling an extern function — `zirk-ir` already has whatever `Call` instruction it uses for user-defined functions; an extern declaration differs only in that it has no Zirk-authored body to lower, so its declaration becomes a codegen-level `declare` (an LLVM external function declaration with C calling convention) instead of a `define`. The checker's job (D3) is making sure no call site reaches this without `unsafe`+`commit`; once past the checker, the call itself is unremarkable.

### D8: Pointer read/write/offset/cast lower to plain LLVM pointer operations; volatile is explicitly not built this slice

`.read()`/`.write(v)` become an LLVM `load`/`store` through the pointer operand (typed by the `Pointer<T>`'s `T`); `.offset(n)` becomes a `getelementptr` in element units; `.offset_bytes(n)` a `getelementptr` over an `i8`-typed view of the same pointer; `.cast<U>()` an LLVM pointer bitcast. `Pointer.from(place)` becomes a new IR instruction (`SlotAddress(SlotId)` or the field-projection equivalent) that in codegen is simply the existing `alloca`/GEP address already computed for that slot/field — no new storage, just exposing an address that already exists.

## Risks / Trade-offs

- **[Risk] The blanket escape rule (D4) rejects some sound programs** (e.g., forwarding a pointer parameter back out as a return value, which cannot itself create a new dangling reference). → Accepted trade-off: soundness now, precision later; nothing that compiles under D4 needs to change when a finer rule replaces it, since D4 is strictly more restrictive.
- **[Risk] The journal (D5) only tracks writes the compiler can see going through a `Store`/`StoreField` inside the lexical block — a write performed by an `extern` call into memory the compiler cannot see (e.g., `memcpy` into a pointer target) is not journaled.** → Explicitly accepted and documented: `MEMORY_AND_UNSAFE_SEMANTICS.md` itself only promises rollback for "Zirk-managed state" and "validated native ranges represented by a bounded mutable view" — an extern call's own writes are exactly the kind of "unknown-effect native library call" §12 already routes through `commit` (irreversible, not rolled back), so this is the normatively correct behavior, not a gap.
- **[Risk] Scope is still large for one change** (new IR type, new checker context stack, new runtime module, integration with existing exception unwinding, new grammar for four constructs). → Mitigation: `NativeSlice<T>`/volatile/native-library-linking are already cut (proposal's "Explicitly out of scope"); if implementation reveals this is still too large for one pass, the natural second cut is separating `extern`+`Pointer<T>` core (D1-D4, D7-D8) from the transactional journal (D5-D6) into two changes — flagged here so whoever implements this can make that call without re-deriving it.

## Migration Plan

Additive over a pipeline that already compiles and runs end to end. Nothing that compiles today changes behavior; `unsafe`/`Pointer`/`commit`/`extern` are rejected keywords/types today and become valid. Rollback: revert the merge; nothing depends on this yet.
