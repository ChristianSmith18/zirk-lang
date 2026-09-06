## Context

The compiler currently treats the exact base-ten type as `Float` and the IEEE 754 family as `BinaryFloatN`. This naming was chosen to keep `Float` as the safe, recommended default, but it clashes with the common expectation that `Float` means a hardware floating-point number. After the `exact-decimal-float` integration, the language now has two distinct, well-defined families, so the names can be aligned with their semantics.

The rename is a surface-only change. The runtime already exposes `zirk_rt_decimal_*` and `zirk_float_*` symbols; no numeric semantics, precision, or overflow behavior changes.

## Goals / Non-Goals

**Goals:**
- Make type names match their numeric semantics: `Decimal`/`Dec` for exact base-ten, `FloatN` for IEEE 754 binary.
- Update every compiler surface that displays or parses these names (lexer, parser, checker, diagnostics, IR, codegen, runtime error strings).
- Update public documentation and the website to reflect the new names and recommend `Decimal` as the default fractional type.
- Provide a clear migration diagnostic for the old `Float` and `BinaryFloat` spellings.

**Non-Goals:**
- Changing the runtime implementation, precision, or the C ABI of decimal/binary helpers.
- Adding or removing type families or widths.
- Changing the literal syntax for decimals or binary floats (the `b` suffix is replaced by `d` for exact decimal and `f`/`fN` for binary floats).
- Modifying the `IntN`/`UIntN` naming convention.

## Decisions

- **Use `Decimal` and `Dec`**: `Decimal` is the canonical name; `Dec` is a short alias similar to `Int`/`UInt`. The alias is accepted in type positions only, not in member access (`Decimal.parse`, not `Dec.parse`), to keep the grammar simple.
- **Use `FloatN` for the binary family**: `Float16`, `Float32`, `Float64`, `Float128`, with `Float` as an alias for `Float64`. This mirrors the familiar `Int`/`Int32` pattern.
- **Single-pass rename in the compiler**: the names are replaced in the lexer token table, the parser's keyword/type maps, the semantic checker's `Base` variants and type name registry, diagnostic strings, and the IR/codegen runtime constant tables. There is no compatibility shim beyond diagnostics.
- **Diagnostic-driven migration**: when a user writes the old name, the compiler emits a `UNKNOWN_TYPE` or `DEPRECATED_TYPE_NAME` diagnostic that explicitly names the replacement. This replaces the current `Float64` migration diagnostic.

## Risks / Trade-offs

- [Risk] This is a breaking change for any code written between the `exact-decimal-float` integration and this rename.  
  → Mitigation: the diagnostic is immediate and actionable, and the `BinaryFloat` → `Float` message will be tested with fixtures.
- [Risk] `Float` as an alias for `Float64` may still be confused with the old exact `Float`.  
  → Mitigation: the alias is limited to the binary family, and the handbook will explicitly contrast `Decimal` (exact) with `Float` (binary shorthand).
- [Risk] Third-party editor tooling or the VS Code extension may need updates to its keyword/keyword list.  
  → Mitigation: the `editor-tooling` capability is marked as impacted and the `keywords` spec will include the new names.

## Migration Plan

1. Implement the name changes in the compiler pipeline.
2. Update existing valid/invalid corpus fixtures and expected outputs.
3. Update the language spec, handbook pages, feature status, and built-in type reference.
4. Run `cargo test --workspace` and the full end-to-end corpus.
5. Sync the website and commit.

## Open Questions

- Should `Float` be a pure alias for `Float64` or a first-class name in error messages? This design treats it as an alias.
- Should `Dec` be allowed in `share type`/`public type` declarations? Yes, same as `Decimal`.
