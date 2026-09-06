## 1. Lexer and parser

- [ ] 1.1 Add `Decimal` and `Dec` as recognized type identifiers in the lexer token table.
- [ ] 1.2 Add `Float16`, `Float32`, `Float64`, `Float128`, and `Float` as recognized type identifiers.
- [ ] 1.3 Remove `BinaryFloat`, `BinaryFloat16`, `BinaryFloat32`, `BinaryFloat64`, `BinaryFloat128` from the recognized type name set and add redirect diagnostics.
- [ ] 1.4 Replace `b`/`bN` binary suffixes with `f`/`fN` and add `d`/`dN` for `Decimal`; keep unsuffixed fractional literals as `Decimal`.

## 2. Semantic checker

- [ ] 2.1 Rename `Base::Decimal` type name in the checker to `Decimal`/`Dec` and `Base::*Float*` variants to `FloatN`.
- [ ] 2.2 Update type mismatch, unknown type, and conversion diagnostics to use `Decimal` and `FloatN`.
- [ ] 2.3 Update scalar static member dispatch (`Decimal.MIN`, `Decimal.MAX`, `Float64.EPSILON`, etc.).
- [ ] 2.4 Update the redirect diagnostic for the old `Float64` spelling to point to `Float64` (binary) and `Decimal` (exact).

## 3. IR, codegen, and runtime

- [ ] 3.1 Update runtime constant and symbol tables that reference `Float`/`BinaryFloat` type names only if used in error strings or metadata.
- [ ] 3.2 Keep `zirk_rt_decimal_*` and `zirk_float_*` runtime C ABI unchanged.

## 4. Tests

- [ ] 4.1 Update sema tests that reference `Float` and `BinaryFloat*` to use `Decimal`/`Dec` and `FloatN`.
- [ ] 4.2 Update parser and lexer tests for the new type names.
- [ ] 4.3 Update end-to-end corpus fixtures and expected outputs for `Float`/`BinaryFloat`.
- [ ] 4.4 Add invalid corpus fixtures for the old spellings and the redirect diagnostic.

## 5. Documentation and website

- [ ] 5.1 Update `docs/handbook` pages that mention `Float` and `BinaryFloat*` to `Decimal`/`Dec` and `FloatN`.
- [ ] 5.2 Update `docs/init/ZIRK_FEATURE_STATUS.md` and `docs/ZIRK_LANGUAGE_SPEC.md` with the new type names.
- [ ] 5.3 Run `cargo test --workspace` and `cargo test -p zirk-cli --test end_to_end`.
- [ ] 5.4 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` and commit the companion `zirk-lang-site` changes.
