## Context

Zirk's scalar surface is split between the grammar (literals are lexed as `i128` / `f64` text plus optional float suffix), the checker (`check_int_literal` always returns `Int32`, `check_float_literal` always returns `Float64`), and the IR (`lower_binary` already inserts conversions for `Int`/`Float` mixes, but only when the `Float` width is `Float64`). This design document describes how to make the language's numeric behaviour match the full set of widths that already exist in the runtime and backend.

## Goals / Non-Goals

**Goals:**

- Allow integer and float literals to take the width implied by their context when the value fits.
- Allow lossless implicit conversions between numeric types at compile time.
- Allow proven-narrow conversions when the compiler can check the range at compile time.
- Make binary arithmetic and `++`/`--` work across all `Int`, `UInt` and `Float` widths by promoting operands to the smallest common type.
- Keep all existing overflow / NaN / domain checks valid for the promoted type.
- Update `String * n` to accept any integer width.

**Non-Goals:**

- Bitwise and shift operators are out of scope; they keep their current same-width requirement.
- Decimal / arbitrary precision numbers are out of scope.
- `Char` numeric operations (e.g. `'a' + 1`) are out of scope.
- User-defined numeric types or operator overloading are out of scope.

## Decisions

### 1. Literal typing is context-directed, not general inference

`check_int_literal` and `check_float_literal` will look at `self.expected_type` when it is a concrete numeric type. If the literal fits that type, they return it. If it does not fit, emit the existing range diagnostic. If there is no expected type, fall back to the current default (`Int32` / `Float64`).

This is the simplest path: it reuses the `expected_type` plumbing already present in `check_expr` and avoids a full unification-style inference engine.

### 2. `Int` ↔ `Float` literal adaptation is allowed both ways

- `mut f: Float = 1;` is allowed because `1` as an exact `Float64` is representable.
- `mut i: Int32 = 1.0;` is allowed because the fractional part is zero and the value fits.
- `mut i: Int32 = 1.5;` is rejected at compile time because `1.5` is not an integer.
- `mut i: Int32 = 1e20;` is rejected because the magnitude does not fit.

This is more restrictive than a general cast: only constants with zero fractional part and representable magnitude pass.

### 3. Conversion matrix follows a numeric lattice

Numeric types are ordered by "can represent every value of the other type" (lossless).

```
            ┌───────────────────────────────┐
            │           Float128            │
            │            Float64            │
            │            Float32            │
            │            Float16            │
  ┌─────────┴──────────┬────────────────────┤
  │      Int128        │       UInt128      │
  │       Int64        │        UInt64      │
  │       Int32        │        UInt32      │
  │       Int16        │        UInt16      │
  │        Int8        │        UInt8       │
  └────────────────────┴────────────────────┘
```

Rules:

- A value may be implicitly converted to any type **strictly above it** on its own branch (widening).
- A signed integer may be implicitly converted to a `Float` only if the float's mantissa can exactly represent every value of that integer type. `Int32` → `Float64` is exact; `Int32` → `Float32` is not exact for large values; `Int32` → `Float16` is almost never exact.
- `UInt` to `Float` follows the same mantissa rule.
- Cross-signedness (`Int` ↔ `UInt`) is allowed only when the source value is provably non-negative for `Int`→`UInt`, or the destination can hold the source range for `UInt`→`Int` (e.g. `UInt8` → `Int16` is lossless, but `UInt32` → `Int32` is not because `UInt32` values > `Int32::MAX` do not fit). When both operands are variables and no constant proof exists, require `as`.
- When no single type can hold both operand families exactly, the expression is rejected; the user must pick an explicit common type with `as`.

### 4. Binary arithmetic promotion is two-phase

1. **Determine the operation's common type** from the two operand types using the lattice above. The common type is the smallest type that can represent every value of both operand types. If none exists, error.
2. **Widen each operand to the common type** in the IR (`SExt` / `ZExt` / `FPExt` / `SIToFP` / `UIToFP`).
3. **Apply the operator and the corresponding checked/NaN trap** at the common type.

For `++`/`--`, the common type is the operand type itself; the literal `1` / `1.0` is created at that width, so no promotion is needed.

### 5. Overflow / error semantics stay the same

All arithmetic remains checked. Promoting `Int8 + Int32` to `Int32` means the result type is `Int32` and the overflow check is emitted for `Int32`. The user gets the same `ArithmeticOverflowError` they would get if they had written the cast manually.

### 6. `String * n` accepts any integer by widening the count

`zirk_str_repeat` takes an `Int32` count. The checker will allow any integer width and the IR will widen the count to `Int32` before the call. Overflow of the count itself is checked at the source width; if it fits in `Int32`, the runtime gets a valid `i32`.

## Risks / Trade-offs

- **Subtle changes to result types**: `Int8 + Int32` used to be an error; now it silently returns `Int32`. This is intentional but may surprise users who expected `Int8`. Mitigation: document the "smallest common type" rule in the handbook and changelog.
- **Float precision confusion**: `Int32 + Float16` will be rejected because `Float16` cannot hold all `Int32` values. Users must widen to `Float32` or `Float64`. This is safer than silently losing precision. Mitigation: diagnostic suggests the smallest safe `Float` target.
- **Constant propagation complexity**: proving `Int32 → Int8` narrowing requires constant folding in the checker. Constant literals and simple constant expressions should be supported; arbitrary variables cannot be narrowed. This is a deliberate limitation (Option A).
- **Backend instruction explosion**: `emit.rs` must handle `Int8 → Int128`, `UInt8 → Int32`, `Int32 → Float64`, etc. LLVM intrinsics already exist; the main risk is choosing the right signed/unsigned conversion.

## Migration Plan

- Implement checker changes behind the existing `Type` / `Base` helpers; no runtime migration.
- Add corpus fixtures that exercise the new rules (`mut a: Int8 = 1;`, `Int8 + Int32`, `Float16 + Int32` error, `String * UInt64`, `Int8++`).
- Run `cargo test --workspace`, `cargo clippy --workspace`, `openspec validate --all --strict`.
- Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark the affected rows complete.
- Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` and commit the website changes.

## Open Questions

- Should `mut a: Int8 = -1;` be allowed? `-1` fits `Int8`; the lexer currently keeps the sign in the `IntLit` value, so `-1` should be fine.
- Should `++`/`--` on `Float` be allowed? IEEE floats do not "overflow" at a fixed max; the current `++` semantics add `1.0`. For `Float` we will emit `Add` without an overflow check, relying on the existing `NaN` trap for `0.0/0.0`-style indeterminacy, not for overflow. If the language later defines `Float` overflow semantics, this can be revisited.
- Should `Int`/`UInt` to `Float` common-type promotion prefer `Float` over `Int` when the float cannot represent the integer exactly? Decision: yes, but if the resulting `Float` cannot represent the integer exactly, reject and require an explicit cast. This keeps the language lossless by default.
