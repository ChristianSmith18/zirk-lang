## 1. Sema: contextual literal typing

- [ ] 1.1 Update `check_int_literal` to use `expected_type` and return a narrower integer/float type when the value fits.
- [ ] 1.2 Update `check_float_literal` to use `expected_type` and return a narrower float/integer type when the value has zero fractional part and fits.
- [ ] 1.3 Add a helper to determine whether a numeric literal value fits in a given `IntWidth`/`FloatWidth` with correct signedness.
- [ ] 1.4 Add `zirk-sema` unit tests for `Int8 = 1`, `Float16 = 1.0`, `Float = 1`, `Int32 = 1.0`, and out-of-range rejections.

## 2. Sema: numeric conversion and promotion

- [ ] 2.1 Implement a numeric conversion lattice (`can_represent_exactly`, `common_numeric_type`) over `Base::Int` and `Base::Float`.
- [ ] 2.2 Update `check_assign` to allow lossless widening and compile-time-proven narrowing.
- [ ] 2.3 Update `check_binary` / `native_arithmetic` to compute a common type and require `as` when no lossless common type exists.
- [ ] 2.4 Update `check_increment` and the `++`/`--` parser path so the synthetic `1`/`1.0` literal matches the operand's exact type.
- [ ] 2.5 Extend `String * n` to accept any integer width with a runtime `Int32` count.

## 3. IR lowering

- [ ] 3.1 Add `Widen`/`Promote` IR operands for converting a numeric value to a larger or common numeric type.
- [ ] 3.2 Update `lower_binary` to widen both operands to the common type before emitting the arithmetic instruction.
- [ ] 3.3 Update `lower_increment` to create the `1`/`1.0` literal with the operand's exact type.
- [ ] 3.4 Update `lower_repeat` to widen any integer count to `Int32`.
- [ ] 3.5 Add `zirk-ir` tests for `Int8 + Int32`, `UInt8 + Int32`, `Int32 + Float64`, and `Int8++`.

## 4. LLVM codegen

- [ ] 4.1 Add emission for `SExt`, `ZExt`, `FPExt`, `SIToFP`, `UIToFP`, and `FPToSI` with correct signed/unsigned and width handling.
- [ ] 4.2 Emit the existing overflow/NaN checks at the promoted common type, not the original operand type.
- [ ] 4.3 Add `zirk-codegen-llvm` emission tests for all mixed-width combinations.

## 5. Runtime / corpus tests

- [ ] 5.1 Add corpus fixtures exercising every integer width with `+`, `-`, `*`, `/`, `++`, `--`, and `String * n`.
- [ ] 5.2 Add corpus fixtures for rejected cases: `UInt64 + Int64`, `Int32 + Float16`, `Int8 = 1000`, `Float = 1.5`.
- [ ] 5.3 Run `cargo test --workspace`, `cargo clippy --workspace`, and `openspec validate --all --strict`.

## 6. Documentation and site sync

- [ ] 6.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark contextual numeric typing and mixed-width arithmetic as completed.
- [ ] 6.2 Update `docs/handbook/13-appendices/07-current-limitations.md` if it mentions numeric `as` requirements.
- [ ] 6.3 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`, review `../zirk-lang-site` diff, and commit the website changes.
