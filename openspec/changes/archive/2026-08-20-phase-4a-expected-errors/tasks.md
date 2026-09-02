## 1. `Base::Never` and `fatalError`

- [x] 1.1 Add `Base::Never` to `zirk-sema`: `accepts` admits it everywhere, `unify` contributes nothing at a union point (D2)
- [x] 1.2 Register `fatalError(message: String): Never` as a compiler-recognized function
- [x] 1.3 `IrType::Never`, `InstKind::FatalError`, and its verification in `verify.rs`
- [x] 1.4 Lowering: `lower_ternary`/`lower_if_expr` without `Store`+`Jump` on the `Never` branch; `lower_expr_as` falls back to `default_value` for a diverging initializer
- [x] 1.5 `zirk_rt_fatal_error` in the runtime, `noreturn` symbol in `zirk-codegen-llvm`
- [x] 1.6 Reject an `enum` with no variants, naming `Never` in the message
- [x] 1.7 Tests: one valid and one invalid case per new rule, plus corpus (`never_and_fatal_error.zrk`, `empty_enum_instead_of_never.zrk`)

## 2. `Result<T,E>` — registration and construction

- [x] 2.1 Audit the existing generic-enum-instantiation machinery (`Iteration<T>`) for arity 2 before committing to the full shape
- [x] 2.2 `register_native_result_enum`: inject `Result` into `self.enums` the same way `register_native_iteration_contracts` does for `Iteration<T>`
- [x] 2.3 `expected_type` (D1): a single-use field on `Checker`, seeding `infer_type_params` from context — needed so `Result.Ok(5)` resolves `E` without any argument determining it
- [x] 2.4 Fix `resolve_written_type` in `zirk-ir/lower.rs`: the enum branch did not consult `reference.arguments` (bug E, D5) — `Result<Int32,String>` written as an annotation resolved to the empty template, not to the instance

## 3. `Result<T,E>` — `match` and exhaustiveness

- [x] 3.1 `expect_pattern_type` did not recognize a pattern against `Base::EnumInstance` (bug A)
- [x] 3.2 `check_variant_pattern` did not substitute field types with the instance's concrete arguments (bug B)
- [x] 3.3 `check_exhaustive` did not resolve the enum id from `Base::EnumInstance` (bug C)
- [x] 3.4 IR: `lower_match` and `declare_pattern_types` indexed `module.enums[]` with the template id instead of the specialized copy (bug D)
- [x] 3.5 Tests: construction and `match` destructuring both variants, corpus (`result_construction_and_match.zrk`)

## 4. `Result<T,E>` — method API

- [x] 4.1 Structural dispatch in the checker (`check_result_method_call`), same pattern as native `to_string()` — no enum method table
- [x] 4.2 `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `unwrap`, `unwrap_error` in IR (`Lowering::result_method`/`lower_result_method_call`)
- [x] 4.3 `opens_blocks` did not know that a call to a `Result` method opens blocks by itself, not only through its arguments (D6) — converted into a `Lowering` method
- [x] 4.4 `unwrap`/`unwrap_error` on the wrong variant invoke `fatalError` (D7), reusing `lower_fatal_error` extracted from `lower_fatal_error_call`
- [x] 4.5 `get_or_else` moved out of scope after colliding with decision D9 (function types have no syntax) — it is not a special case of `Result`, it is a general language restriction (D4)
- [x] 4.6 Unit tests for the seven methods, plus invalid cases (unknown method, wrong argument type, wrong arity)

## 5. Mandatory consumption

- [x] 5.1 `require_result_consumed`: an expression statement whose type is a `Result` instance is an error (`E0433`)
- [x] 5.2 `_ = expr;`: `check_assign` recognizes the name `_` as an explicit discard, without resolving it as a binding
- [x] 5.3 IR: `lower_assign` discards the value of `_ = expr;` without looking for a slot that was never declared
- [x] 5.4 Tests: one valid case (`_ = load();`) and one invalid case (discarded statement) per path, plus invalid corpus (`discarded_result.zrk`)

## 6. Closeout

- [x] 6.1 `cargo test --workspace`, `cargo clippy --workspace --all-targets`, `cargo fmt --check` all green
- [x] 6.2 `design.md`: `## Decisions` section completed (D1–D8)
- [x] 6.3 `proposal.md`: scope corrected to move `get_or_else` alongside the generic combinators
