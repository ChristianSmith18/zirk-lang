## Why

`fase-4e-unsafe-pointer-extern` (merged) delivered raw `Pointer<T>` but explicitly deferred the safer, spec-preferred surface: `MEMORY_AND_UNSAFE_SEMANTICS.md` §8 says to "prefer bounded views over a loose pointer/length pair," and `zirk-memory-safety`'s own normative text already requires validated `NativeSlice<T>`/`NativeSliceMut<T>` views with bounded extent and lifetime, plus an already-normative escape scenario ("Native view escapes its borrow") that `fase-4e-unsafe-pointer-extern`'s own design explicitly says it is giving `Pointer<T>` "the same treatment before `NativeSlice<T>` itself is built." That type has zero compiler trace today.

## What Changes

- New types `NativeSlice<T>` (read-only) and `NativeSliceMut<T>` (read-write), constructed only from a validated `Pointer<T>` + length via `pointer.as_slice(length): Result<NativeSlice<T>, NativeError>` / `pointer.as_slice_mut(length): Result<NativeSliceMut<T>, NativeError>` (both `unsafe`, matching `Pointer<T>`'s own operations). Construction validates nullability, alignment, extent, provenance where available, and mutation rights.
- **New general grammar**: `expr[index]` did not exist anywhere in the language before this change (found during implementation scoping, not anticipated at proposal time — `[`/`]` were lexed tokens with no parser production, and `Array<T>`/`List<T>` themselves are unbuilt Phase 7 collections, so there was nothing to reuse). This change adds `Expr::Index` as a genuine new postfix expression (parses for any receiver; the checker restricts which receiver types actually support it, currently only `NativeSlice<T>`/`NativeSliceMut<T>`, via a small extensible dispatch table so Phase 7 collections can register their own support later without further grammar changes). Bounds-checked indexing and iteration stay active in *safe* code once a view exists — the view itself does not require `unsafe` to read/write through, only to construct, mirroring `MEMORY_AND_UNSAFE_SEMANTICS.md`'s own framing ("Bounds checks remain active in safe operations over the resulting view").
- `.length`, `.is_empty` (already documented in `docs/handbook/11-reference/13-type-member-index.md` ahead of implementation).
- Dependent-reference escape checking: a `NativeSlice<T>`/`NativeSliceMut<T>` whose validated owner ends in the function cannot be returned, stored past the owner's scope, or captured by a closure — the already-normative "Native view escapes its borrow" scenario in `zirk-memory-safety`, reusing the same conservative rejection strategy `fase-4e-unsafe-pointer-extern` already built for `Pointer<T>` (any static occurrence as a return value, field-write source, or closure capture is rejected, regardless of whether the specific case is hypothetically safe).
- The view never owns or frees the storage it observes — no interaction with the collector beyond the ordinary lifetime of whatever `Pointer<T>` it was built from.

### Explicitly out of scope

- **Automatic pinning of movable managed storage for a native borrow** (`MEMORY_AND_UNSAFE_SEMANTICS.md` §5's `Pin<T>` machinery) — the collector this compiler has (`fase-4e-colector-mark-sweep`) is already non-moving, so nothing needs pinning yet; this change's escape check is a static compile-time rule, not a runtime pin. If a moving collector strategy is ever adopted, pinning becomes load-bearing and is its own future change.
- **Volatile access through a view** (`.read_volatile()`/`.write_volatile()`) — those exist on `Pointer<T>` only per the current spec text; extending them to views is not requested by any existing requirement.
- **Untagged native-union member access** — no union type exists in Zirk yet.
- **A native-library-linking manifest** — `extern "C" fn` resolution stays exactly as `fase-4e-unsafe-pointer-extern` left it (system linker only); this change adds a data view type, not a linking mechanism.

## Capabilities

### New Capabilities
(none)

### Modified Capabilities
- `zirk-memory-safety`: `NativeSlice<T>`/`NativeSliceMut<T>` construction, indexing, and the "Native view escapes its borrow" scenario move from specified-only to delivered (requirement text reaffirmed, checked against existing wording during design).
- `zirk-type-system`: `NativeSlice<T>`/`NativeSliceMut<T>`'s element-type restriction and capability set (currently named alongside `Weak<T>`/`Pointer<T>` in the type-system spec's list without their own dedicated requirement) get one, following the same pattern `fase-4e-unsafe-pointer-extern` used for `Pointer<T>` and `fase-4e-weak` used for `Weak<T>`.
- `zirk-grammar`: new requirement for `expr[index]` as a general postfix expression (design D5) — genuinely new grammar surface, not previously specified anywhere, found necessary only once implementation started.

## Impact

- Affected code: `crates/zirk-ast`/`crates/zirk-parser` (new `Expr::Index` node and its grammar production, precedence, and place/assignment-target classification — design D5), `crates/zirk-sema` (`Base::NativeSlice`/`Base::NativeSliceMut` types, `.as_slice`/`.as_slice_mut` typing on `Pointer<T>`, the escape-check reuse from `Pointer<T>`'s existing conservative rule, an extensible receiver-type-indexed dispatch table for indexing/`.length`/`.is_empty` typing), `crates/zirk-ir` (view construction/bounds-checked-index instructions), `crates/zirk-codegen-llvm` (lowering: a view's runtime representation is a `(pointer, length)` pair; bounds checks emitted at every index), `crates/zirk-runtime` (validation helpers for construction — alignment/extent/nullability checks).
- Public documentation: `docs/handbook/13-appendices/07-current-limitations.md`/`12-feature-status.md` "`NativeSlice<T>`/volatile access" line updated (`NativeSlice<T>` delivered; volatile access remains open, unchanged, on `Pointer<T>` only). `docs/handbook/05-native-and-low-level/02-importing-c.md` and `03-exporting-zirk.md` already show `NativeSlice<T>` usage ahead of implementation — reconcile against the real implementation once built. `../zirk-lang-site` sync required once this lands.
- No breaking changes: `NativeSlice<T>`/`NativeSliceMut<T>` are new types; nothing that compiles today changes behavior.
