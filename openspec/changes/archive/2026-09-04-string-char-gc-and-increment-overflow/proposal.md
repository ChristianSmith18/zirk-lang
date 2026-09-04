# String/Char GC and increment overflow

## Why

`String` and `Char` values are heap-allocated opaque runtime handles, but they are not currently managed by the mark-sweep collector. Every `to_string()`, concatenation, `String * n`, grapheme slice, and string literal materialization leaks two heap objects (the handle and its bytes) through `Box::into_raw`. This makes any loop that builds strings grow without bound, which is exactly what `hello.zrk` demonstrates.

At the same time, prefix and postfix `++`/`--` in expression position are lowered as raw `Binary` additions/subtractions without the `CheckedArithmetic` guard used by ordinary `+`/`-`/`*`. An `Int32` at `2147483647` wrapped to `-2147483648` when used as `x = i++`, hiding an overflow that the language already promises to report.

Both issues are closed in one change because they share the same runtime/lowerer surface: making `String`/`Char` collected requires treating them as managed references in the IR, and fixing increment overflow requires the same `lower_increment` path that already lives next to `String`-producing expressions.

## What Changes

- **Make `String` and `Char` collector-managed references.**
  - `zirk-runtime` allocates string/char objects through `zirk_rt_alloc`, with a dedicated `zirk_rt_string_descriptor` and inline or external bytes, instead of leaking `Box` allocations.
  - `crates/zirk-ir/src/ir.rs` treats `IrType::String` and `IrType::Char` as managed references, so synthetic-slot spilling and GC root enumeration cover them.
  - `crates/zirk-codegen-llvm/src/emit.rs` emits string/char fields as GC reference paths.
  - `crates/zirk-runtime/src/clone.rs` recognizes string objects and shares their immutable handles instead of deep-copying bytes.

- **Fix `++`/`--` overflow in expression position.**
  - `crates/zirk-ir/src/lower.rs` lowers `Expr::Increment` through `Self::checked_int_arithmetic` (or `emit_checked_binary`) so prefix/postfix increment and decrement throw `ArithmeticOverflowError` on signed/unsigned overflow, matching `+`/`-`/`*`.

- **Tune and verify GC behavior under string pressure.**
  - Add corpus fixtures that run bounded loops building and discarding strings, asserting stable peak RSS.
  - Add corpus fixtures for `i++`/`--i` overflow that terminate with `ArithmeticOverflowError`.
  - Update public status documents to reflect that `String`/`Char` are reclaimed and that `++`/`--` are checked.

## Capabilities

### New Capabilities

None. This change realizes existing memory-safety and scalar guarantees rather than introducing new language surface.

### Modified Capabilities

- `zirk-memory-safety`: Clarify that the strategy-neutral automatic memory guarantee covers all runtime-allocated opaque handles, specifically `String` and `Char`, and that unreachable string/char values are reclaimed by the same collector as ordinary objects.
- `zirk-scalars`: Clarify that prefix and postfix `++`/`--` are arithmetic operations subject to the same checked overflow rules as `+`/`-`/`*` on every integer width and signedness.

## Impact

### Code

- `crates/zirk-ir/src/ir.rs` — `IrType::is_managed_reference` includes `String`/`Char`.
- `crates/zirk-ir/src/lower.rs` — `lower_increment` uses checked arithmetic; string results are spilled like other managed references.
- `crates/zirk-codegen-llvm/src/emit.rs` — `gc_reference_paths` treats `String`/`Char` as leaf managed references.
- `crates/zirk-runtime/src/string.rs` — all constructors allocate through `zirk_rt_alloc`; `borrow` accounts for the GC object header.
- `crates/zirk-runtime/src/collector.rs` — new `zirk_rt_string_descriptor` static with no GC-referenced fields.
- `crates/zirk-runtime/src/clone.rs` — special-cases string objects to share handles.
- `crates/zirk-cli/tests/corpus/valid/` and `invalid/` — new fixtures for string collection and increment overflow.

### Public content

- `docs/init/ZIRK_FEATURE_STATUS.md` — update the `Non-moving mark-sweep GC` row and `String`/`Char`-related notes.
- `docs/handbook/13-appendices/07-current-limitations.md` — remove or rephrase any wording that implies strings/chars are not reclaimed.
- `docs/decisions/proximos-pasos-fase-4.md` — update section 6.1 to reflect the decision to integrate `String`/`Char` into the collector.
- `../zirk-lang-site` — sync handbook and status evidence via `./scripts/sync-website-content.sh` after the zirk-lang commits land.

### Breaking changes

None at the language surface. Runtime string handles remain opaque pointers, so ADR-005 is preserved. Performance may change (fewer leaked allocations, more frequent GC collections on string-heavy workloads).
