## Context

Every earlier phase worked with exactly the scalars it needed and nothing more: Phase 1 fixed `Int32`/`Boolean`/opaque `String` because the minimal pipeline asked for no more, and no later phase touched them again. `ZIRK_LANGUAGE_SPEC.md` section 3 always promised the complete family; this is the first phase that builds it.

The central risk is one of real scope, not conceptual difficulty: `IrType::Int32` (and its checker equivalent, `Base::Int32`) is written as if it were the only possible integer in hundreds of sites across `zirk-sema`, `zirk-ir`, and `zirk-codegen-llvm` — every `match` on a type, every conversion to LLVM, every arithmetic rule. Generalizing to ten integer widths plus `Float` is not adding new variants on the side; it is reviewing each of those sites to confirm it stops assuming `Int32`.

Standing ADRs that constrain this phase:

| Decision | ADR |
|---|---|
| `String` is opaque behind the runtime boundary | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| `String` identity, equality, and normalization | [ADR-011](../../../docs/decisions/ADR-011-identidad-e-igualdad-de-string.md) |
| IR shape: three addresses, basic blocks, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| Object layout: separate header, inline records/value classes | [ADR-012](../../../docs/decisions/ADR-012-layout-de-objetos.md) |
| Dispatch shape: direct by default, a table only when needed | [ADR-013](../../../docs/decisions/ADR-013-forma-del-despacho.md) |

## Goals / Non-Goals

**Goals:**

- The ten signed and unsigned integer widths, with checked arithmetic as strict as what `Int32` already has.
- The complete `Float` family, with no valid `NaN`, with explicit infinities.
- `Char` as a real Unicode grapheme, not an alias for a 32-bit integer.
- Deep contextual conversion (`Float(3 / 4)`) and section 3's implicit/explicit widening rules.
- Bitwise and shift operators.
- The `to_string()` contract (`ZIRK_STDLIB_SPEC.md` section 3), `print`/`println` routing through it, and string interpolation (`"{expr}"`, same section).

**Non-Goals:**

- `Decimal`, temporal types, anything from `std.collections` — this phase does not ask for them.
- `format`/`format_dynamic` with specifiers (`:name`, `:0`, `:name|format`) and `Regex` — `ZIRK_STDLIB_SPEC.md` section 7 leaves them for when `std.text` exists as a module, which is library surface, not language surface.
- Explicit wrapping/saturating arithmetic as an operation — the spec does not ask for it yet outside of contextual conversion.
- Any Unicode normalization or algorithm beyond recognizing grapheme boundaries for `Char`.

## Decisions

### D1 — An integer is a width plus a signedness, not ten distinct types in the IR

`IrType::Int32` is generalized to something with two parameters —width and signed/unsigned— instead of multiplying into ten enum variants. LLVM already models any integer width natively (`IntType`), so the backend does not gain real complexity: it gains a parameter where it used to have a constant.

The cost is paid in the checker and in `lower.rs`: every site that today compares directly against `Base::Int32`/`IrType::Int32` needs review — some must generalize to "any integer," others (the ones that today assume 32 bits because there was never another option, such as the size of an enum discriminant) need an explicit decision about which width to use. This is audited exhaustively when implementation starts, not assumed to be covered by "generalizing the type."

**Discarded alternative:** one IR variant per width (`Int8`, `Int16`, …). It multiplies every exhaustive `match` by ten with no gain — the width is data, not a distinct instruction shape.

**Audit (task 1.1, done):** count of sites naming `Base::Int32`/`IrType::Int32` directly, per file:

| File | Occurrences | Nature |
|---|---|---|
| `zirk-sema/src/types.rs` | 4 | `Type::INT32` (constant), `has_default`, `integer_range`, `sort_bases` — genuinely per-type logic, not a mechanical rename |
| `zirk-sema/src/checker.rs` | ~25 sites with `match ... .base { ... }` including a `Base::Int32` arm | Mixed: most are "is this an integer?" (generalizes with no further work), but `native_arithmetic` (the native operator table) and `require_printable` need to explicitly decide which widths participate and with what conversion rules |
| `zirk-ir/src/ir.rs` | 6 | Type definitions/helpers — mechanical |
| `zirk-ir/src/lower.rs` | 30 | `Base`→`IrType` conversion and operand type bookkeeping — mostly mechanical, except the overflow check, which needs the real width |
| `zirk-ir/src/verify.rs` | 4 | IR invariant checking — mechanical |
| `zirk-codegen-llvm/src/emit.rs` | **1** | `IrType::Int32 => context.i32_type().into()` — the backend itself is cheap: LLVM already supports any width, this is the only site that translates the type |

**Audit conclusion:** the cost is not evenly distributed. The LLVM backend is almost free (one site). The real cost is in `zirk-sema/checker.rs`: `Base::Int32` today is not "the integer," it is "the only integer that exists," so the native arithmetic table, printing, the default value, and the union interning order assume this structurally, not by oversight. Generalizing `Base::Int32` to `Base::Int { width, signed }` (or equivalent) is a change the Rust compiler itself makes impossible to leave half-done — every exhaustive `match` on `Base` that does not cover the new arm fails to compile — so there is no real risk of a forgotten site, only of underestimating how much **decision** work (not mechanical work) each site needs.

**Scope decision (task 1.2):** a single atomic migration — parameterize `Base::Int32`/`IrType::Int32` once, with all ten widths already declared from the start — instead of generalizing first and adding widths later. Adding widths in two passes would pay the cost of reviewing each site twice; the compiler's exhaustiveness already guarantees no site is left half-done in a single pass.

### D2 — `Float` forbids `NaN` in the type, it does not discard it afterward

An operation that in IEEE 754 would produce `NaN` (`0.0 / 0.0`, the root of a negative, etc.) turns into a controlled error at the point where it occurs, the same way integer overflow already has since Phase 1 — the `NaN` bit pattern is not allowed to propagate and be checked at the end. Infinities, on the other hand, are valid, explicit values.

This means every `Float` operation that LLVM would expose with standard IEEE 754 semantics needs its own check before or after the `fdiv`/`fsub`/etc. — it is not a validation that can be centralized in a single spot, because each operator can produce `NaN` for a different reason.

**Discarded alternative:** representing `NaN` as a valid value and rejecting it only on observation (comparison, printing). The spec is explicit ("no valid NaN"), and waiting until the observation point lets a `NaN` live arbitrarily long in the program before failing, with the diagnostic pointing far from its real cause.

### D3 — Checked arithmetic per width, reusing `Int32`'s mechanism

Phase 1 already built the checked-overflow path for `Int32` (checker plus runtime support). Every new width reuses exactly that mechanism, parameterized by width and signedness — not a parallel implementation. Signed and unsigned differ in which overflow comparison they use (LLVM exposes `*.with.overflow` intrinsics for both), not in the shape of the check.

## Risks / Trade-offs

- **`Char` as a real grapheme does not, in general, fit in a fixed-size scalar.** An extended grapheme cluster (a base plus combining marks, or an emoji sequence joined by ZWJ) can occupy arbitrarily more than 4 UTF-8 bytes. Section 3 is explicit: "even when composed of multiple code points and bytes." This is a **real open question, not a decision already made** — see below. Provisional mitigation: design `Char` as an opaque view backed by the runtime, with the same principle ADR-005 already applied to `String` (an opaque boundary, with no direct LLVM representation the frontend needs to know about), instead of forcing it into some fixed width before having the answer.

- **Generalizing `IrType::Int32` is this phase's biggest scope risk.** There is no way to measure how many sites assume 32 bits without auditing them one by one; any estimate made before that audit is unreliable. Mitigation: the audit is the first implementation task, before writing any new width — if the real cost exceeds expectations, scope is cut (e.g. delivering only `Int64`/`UInt64` first) rather than forcing a deadline.

- **Deep contextual conversion interacts with Phase 3's operator contracts, and the spec does not say explicitly how.** `Float(3 / 4)` rewrites `/` before lowering it when the operands are native; with a user type overloading `_divide` in play, it is not written whether the context propagates through the call to the reserved method or stops at the operand that triggers it. This is decided during implementation, guided by the precedent that operator contracts **do not** alter precedence or arity (section 4) — the context should not alter which method is called either, only which values it receives.

- **`to_string()` is a bigger piece than "one more function."** It needs: the reserved contract itself, a native type implementing it for every scalar (including the ten new integers and `Float`), dispatch from `println`/`print`, and interpolation's desugaring calling it for every `{...}` expression. In size, it is comparable to what operator contracts were in Phase 3 — it is treated as its own piece of the task plan, not as a detail of interpolation.

## Migration Plan

Additive on top of a pipeline that works end to end, the same as previous phases. `Int32`/`Boolean`/`String` do not change observable behavior; what changes is that they stop being flagged as special cases in the checker.

The current `require_printable` diagnostic — which only accepts `Int32`/`Boolean`/`String` in `println` — is retired once `to_string()` exists for every native and user type.

Rollback: revert the merge. No later phase depends on this one yet.

## Open Questions

- **How is `Char` represented in the IR and at runtime?** See the risk above. An extended grapheme cluster has no maximum size guaranteed by the Unicode standard. Options to evaluate before committing: (a) an opaque view backed by the runtime, like `String`; (b) an inline value with small-buffer optimization and an escape for the rare case that exceeds the buffer; (c) restrict `Char` to a single code point for this phase and note the extended-grapheme case as explicit debt toward a later phase — which contradicts section 3 as it is written today, so it would first need a spec correction, not just an implementation one.

- **Does the deep-conversion context cross an operator overloaded via contract?** See the corresponding risk above. Proposal to confirm during implementation: no — the context converts the operands before the reserved method is called, and the reserved method sees its parameters with the type it already declared, with no knowledge of the context that produced them.

- **What is the printing contract's exact name?** `ZIRK_STDLIB_SPEC.md` section 3 says `to_string(): String` in prose; `ZIRK_LANGUAGE_SPEC.md` section 4 documents the underscore-prefixed reserved-method convention (`_add`, `_subtract`) for operator contracts, but `to_string` is not an operator — it remains to confirm whether it follows that same convention (`_to_string`) or is an ordinary public method with no reserved prefix, given that it is also called directly by the user (`valor.to_string()`), not only implicitly by the compiler as `_add` is.
