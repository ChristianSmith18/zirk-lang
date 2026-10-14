# Tasks: fix interpreter parity gaps

## 1. Void-call and effect-instruction contract

- [x] 1.1 In `zirk-interpreter`, derive `caller_value` only when `instruction.ty != IrType::Void` for `Call`, `CallClosure`/`CallCallable`, `CallVirtual`, and `CallContract`; verify a standalone `Void` function call and a `Void` method call run without `non-void call returned no value` in an interpreter unit test
- [x] 1.2 In `zirk-ir/src/verify.rs`, add a rule rejecting `result: Some` on `Void`-typed instructions; verify a hand-built module with `result` on a `Void` call fails verification with a structured diagnostic
- [x] 1.3 In `zirk-ir/src/lower.rs`, migrate `Void` call sites (`lower_expr_for_effect`, `lower_method_call`, throws-check paths) from `emit` to `emit_effect`; verify `cargo test -p zirk-ir` passes and lowered `Void` calls carry `result: None`
- [x] 1.4 Migrate effect-instruction emit sites for `ListAdd`, `ListInsert`, `MapSet`, `SetAdd` (and any other `Ok(None)` instructions found at `lower.rs` ~14209, 14666, 14671, 17194, 17214) to `emit_effect`; verify `l.add`, `l.insert`, `m.set`, `s.add` each execute in the interpreter without `instruction result was not materialized`
- [x] 1.5 Audit remaining `emit(..., IrType::Void, ...)` sites and fix any stragglers; verify `rg 'emit\(.*IrType::Void'` in `lower.rs` returns no statement-position uses

## 2. `Retype` and exception handling

- [x] 2.1 Implement the `Retype` arm in `execute_instruction` (pass-through of the resolved operand, relying on the already-emitted runtime type check for narrowing); verify a `catch Type(e)` handler that reads `e` executes in the interpreter
- [x] 2.2 Verify `as`/`as?` downcasts and subclass→supertype/interface argument coercion work in the interpreter; add a regression test for each (downcast success, `as?` returning `null`, `greet(d)` with `s: Speak` interface param)
- [x] 2.3 Render the throwable's `message` field in `uncaught_exception` instead of `{:?}` on the heap value; verify an uncaught `throw` produces the semantic message and no `Heap(HeapId(...))` substring

## 3. Extern parity matrix and browser intrinsics

- [x] 3.1 Generate the parity matrix: diff all `extern` declarations registered by lowering against `is_browser_intrinsic`, classify each of the ~171 missing names as implement-pure / native-only / out-of-scope, and commit the matrix as a checked-in document or test fixture; verify the matrix enumerates every missing name
- [x] 3.2 Implement `zirk_rt_decimal_*` intrinsics (add/sub/mul/div/rem/cmp/round/abs/sqrt/parse) via `zirk-runtime-core`; verify `10 / 3`, `2.5 + 1.25`, decimal literals, and `x == y` on `Decimal` all run in the interpreter
- [x] 3.3 Implement `zirk_int_*` and `zirk_float_*` member intrinsics (`abs`, `is_even`, `checked_pow_*`, `floor`, `sqrt`, `Float64.parse`, …); verify `x.abs()`, `2 ** 10`, `f.floor()` run in the interpreter
- [x] 3.4 Implement `zirk_str_*` and `zirk_regex_*` intrinsics (`split`, `lines`, `chars`, `find`, `find_all`, `split`); verify `s.split(",")`, `s.lines()`, regex find/split run in the interpreter
- [x] 3.5 Implement `zirk_duration_*` and pure `zirk_rt_{date,time,datetime}_*` intrinsics (parse/format/arithmetic, not `today`/`now`); verify `Date.parse("2024-01-01")` and `duration * 2.0` run in the interpreter, and `Date.today()` still fails preflight with a capability diagnostic
- [x] 3.6 Implement the remaining pure externs (`zirk_char_*`, `zirk_rt_array_from_list`, `zirk_rt_map_sorted_keys`, `zirk_rt_set_sorted_elements`); verify each with an interpreter test

## 4. Language-level: numeric comparisons and `Decimal` ordering

- [x] 4.1 In `zirk-sema`, add `Base::Decimal` to `expect_comparable`; verify `price > qty` on two `Decimal` values compiles with no diagnostic
- [x] 4.2 Replace the "same numeric type" comparison check with smallest-common-type widening (matching the arithmetic rule); verify `for mut i = 0; i < xs.length; i++` compiles and runs natively and in the interpreter, and `Decimal` vs `Float64` comparison is still rejected
- [x] 4.3 Update the handbook so `Array`, `List`, `Map`, and `Set` document `length` with consistent signedness (`UInt64`); verify `docs/handbook/02-handbook/12-collections/*.md` agree

## 5. Conformance fixtures and verification

- [x] 5.1 Add browser conformance fixtures: `void-call`, `void-method-call`, `list-add`, `map-set`, `set-add`, `typed-catch`, `downcast`, `decimal-ops`, `decimal-compare`, `int-division`, `length-loop`, `uncaught-throwable-message`; verify each fixture passes through both engines via the fixture runner
- [x] 5.2 Run the full suite: `cargo test -p zirk-ir -p zirk-sema -p zirk-interpreter -p zirk-browser`; verify no regressions
- [x] 5.3 Re-run the original `ejemplo.zrk`, `01_length.zrk`, and the decimal repro through the browser-equivalent pipeline; verify all three produce the native-identical output
