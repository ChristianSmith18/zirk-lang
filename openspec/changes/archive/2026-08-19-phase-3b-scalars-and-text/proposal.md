## Why

Phase 1 implemented a single integer width (`Int32`), a `Boolean`, and an opaque `String` — the minimum needed for the pipeline to compile end to end. Everything else `ZIRK_LANGUAGE_SPEC.md` section 3 promises about scalars —the rest of the integer widths, the `Float` family, real `Char`, deep contextual conversion— never had an assigned phase. `ZIRK_ROADMAP.md` fixes it as Phase 3b:

> The remaining integer widths... The binary floating family... `Char` as exactly one Unicode grapheme... Deep contextual conversion... Bitwise and shift operators... String interpolation.

They arrive together because they depend on each other: contextual conversion means nothing without `Float`, `Float` literals mean nothing without the complete family, and interpolation needs the `to_string()` contract that this phase also closes. Splitting them would force implementing each one twice.

### A dependency the roadmap assumes is resolved, and it is not

The roadmap describes interpolation as something that "needs the `to_string()` contract Phase 3 defines." Audited against the code: **Phase 3 never defined that contract.** `require_printable` in `zirk-sema/src/checker.rs` still accepts only `Int32`, `Boolean`, and `String` in `stdout.println`, with a hint message that literally says "`to_string()` becomes a trait in Phase 3" — an expectation that was left unclosed. No enum, no class, and no record can be printed today without converting its fields by hand.

This phase absorbs that debt: the `to_string()` contract (or whatever name `ZIRK_STDLIB_SPEC.md` section 3 fixes) is implemented here, as a real precondition of string interpolation, not as something already available that is assumed.

## What Changes

### Full integer widths

- Signed: `Int8`, `Int16`, `Int64`, `Int128` (`Int32` already exists). `Int`/`Integer` remain aliases of `Int32`.
- Unsigned: the complete `UInt8`…`UInt128` family, entirely new.
- Checked arithmetic on every width: overflow is a controlled error, the same as it already is for `Int32` (Phase 1); explicit wrapping/saturating/checked arithmetic is out of scope for this phase unless auditing the spec shows another section requires it.
- Implicit safe widening where it is not ambiguous; conversion with or without sign, and any lossy conversion, always explicit (section 3).

### The `Float` family

- `Float16`, `Float32`, `Float64`, `Float128`, with `Float` as an alias of `Float64`.
- Explicit positive and negative infinities; **no** valid `NaN` — an operation that would produce one is a controlled error, not a special value that propagates.
- Fractional literals (`1.5`, scientific notation `1e2`), defaulting to `Float64` when no context says otherwise.
- Mixed integer/`Float` arithmetic produces `Float` (section 3, already documented, not implemented because `Float` does not exist yet).

### `Char`

- Exactly one Unicode grapheme, even when it spans several code points — not a `UInt32` under a different name.
- What finally allows iterating a `String` grapheme by grapheme (`for c in s`, a debt noted since Phase 2 and again in Phase 3's `for ... in`).

### Deep contextual conversion

- An explicit constructor (`Float(3 / 4)`, `String("value=" + 42)`) establishes a domain for the whole tree of directly compatible operators inside it, converting the operands before operating — not after. The context does not mutate the operands nor cross into the body of a called function (section 3).

### Bitwise and shift operators

- `&`, `|`, `^`, `~`, `<<`, `>>` on integer types, signed and unsigned, at the precedence levels the spec fixes.

### The `to_string()` contract, and with it interpolation

- The reserved contract that `ZIRK_STDLIB_SPEC.md` section 3 requires, implementable by any user type — class, record, enum — with the same reserved-contract discipline Phase 3 fixed for operators (`ZIRK_LANGUAGE_SPEC.md` section 4: overloading only through a language contract).
- `stdout.println`/`stdout.print` now route through it instead of the closed list of three types they have today.
- String interpolation (the exact syntax is fixed by `ZIRK_LANGUAGE_SPEC.md`; audit against section 1/7 while designing), which uses it to convert each interpolated expression.

### Explicitly out of scope

- **`Decimal`** — section 3 marks it as "may be a later stdlib type," not part of this phase.
- **Temporal types** (`Date`, `Time`, `Duration`, etc., section 8.1) — their own family, with no phase assigned yet in the roadmap.
- **Explicit wrapping/saturating arithmetic** beyond what contextual conversion already covers, unless auditing the spec during design shows this phase requires it.
- **Anything related to collections** (`List<T>` and friends, Phase 7) — a `Char` iterating a `String` does not depend on them.

## Capabilities

### New Capabilities

- `zirk-scalars`: the full integer widths, the `Float` family, `Char`, and the conversion rules (safe implicit, lossy explicit, deep contextual) among all of them.

### Modified Capabilities

- `zirk-grammar`: `Float`/width literals with a suffix if the spec requires it, bitwise/shift operators, interpolation syntax.
- `zirk-type-system`: the complete family of numeric types and their conversion rules; the `to_string()` contract.
- `zirk-ir-lowering`: representation of every integer width and of `Float`, lowering of contextual conversion, of bitwise/shift, and of interpolation.
- `zirk-native-codegen`: codegen for arithmetic per width, for `Float`, and for dispatching to `to_string()` from `println`/interpolation. The runtime primitive that writes a `String` to standard output (`zirk-runtime-io`) does not change — what changes is what gets converted to `String` before reaching it.

## Impact

**Modified crates:** `zirk-lexer` (literals), `zirk-parser`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime`. None new.

**No new external dependencies** — LLVM already models every integer width and every `Float` width natively; none of this requires custom floating-point software.

**Risks:**

- **The `to_string()` debt is bigger than the roadmap assumes.** It is not "it already exists, just use it": it is an entirely new piece (reserved contract + dispatch + integration with `println` + integration with interpolation). Its real scope is audited while writing `design.md`, before committing to a shape.
- **An invalid `NaN` is a strong constraint that touches every `Float` operation.** Every operation that in IEEE 754 would produce `NaN` (`0.0 / 0.0`, `sqrt(-1)`, etc.) needs to turn into a controlled error instead of letting the `NaN` bit pattern propagate — that is work in every operation, not a check at the end.
- **Integer widths multiply Phase 1's checked-arithmetic surface.** What today is a single overflow path for `Int32` becomes one per width, signed or unsigned — ten combinations instead of one. It is audited whether the checker/IR already generalize this or assumed `Int32` at some undocumented point.
- **Deep contextual conversion interacts with Phase 3's operator contracts.** `Float(3 / 4)` rewrites the `/` tree before lowering it; with `_add`/`_divide` in play for a user type, it must be decided whether the context propagates through an overloaded operator or stops there — the spec does not say so explicitly, so it is an open design question.
