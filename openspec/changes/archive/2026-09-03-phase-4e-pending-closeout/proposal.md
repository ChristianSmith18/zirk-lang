## Why

Phase 4e of the Zirk roadmap left three closely related `unsafe`/memory pieces with documented limitations. Closing them now is the lowest-risk way to finish the unsafe pointer, journal, and indexing surfaces before attempting the larger dependent-reference and pinning design later in Phase 4e or beyond.

## What Changes

- Complete `unsafe {}` journal rollback for `return`, `break`, and `continue` exits.
- Extend `Pointer.from` to accept fields of `record` and `value class` storage, not only `class` slots.
- Add read-only `String[index]` indexing that returns a single grapheme `Char`.
- Leave dependent references and automatic bounded native pinning as an explicit out-of-scope follow-up because they require a separate design spike and runtime work.

## Capabilities

### New Capabilities

- *None.* All three features are refinements of existing capabilities.

### Modified Capabilities

- `zirk-memory-safety`: require journal rollback when `return`, `break`, or `continue` exits an `unsafe { ... }` block without an exception.
- `zirk-data-types`: allow `Pointer.from(place)` where `place` is a field or slot of a `record` or `value class` whose underlying storage is addressable.
- `zirk-ir-lowering`: emit `JournalRollback` on all non-exceptional exits from `unsafe` blocks.
- `zirk-native-codegen`: emit `getelementptr` for pointer-to-field inside value-type storage and expose `zirk_str_grapheme_offset` for runtime string indexing.

## Impact

- `crates/zirk-ir/src/lower.rs` — new exit-cleanup path combining `try` and `unsafe` frames.
- `crates/zirk-sema/src/checker.rs` — static validation for `Pointer.from` value-type lvalues.
- `crates/zirk-ir/src/ir.rs` and `crates/zirk-ir/src/lower.rs` — new `StringGraphemeOffset` instruction.
- `crates/zirk-runtime/src/string.rs` — new `zirk_str_grapheme_offset` runtime helper.
- `crates/zirk-codegen-llvm/src/emit.rs` — value-layout GEP path for `PointerFromField`.
- CLI corpus and `crates/zirk-ir/tests/lowering.rs` — new fixtures and IR tests for all three features.
- `docs/init/ZIRK_ROADMAP.md` and `docs/init/ZIRK_FEATURE_STATUS.md` — remove or update the listed limitations.
