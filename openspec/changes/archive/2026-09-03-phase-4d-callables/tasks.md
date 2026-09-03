> **Shortcut for this checkpoint:** `IrType::Callable`, `MakeCallable`,
> `CallCallable`, and `.clone()` on callables are implemented end-to-end.
> The capture block is allocated by `zirk_rt_alloc_callable`, tracked by the GC
> through per-closure descriptors, and deep-copied by `zirk_rt_clone_callable`.
> Named functions, capture-less lambdas, and captured lambdas can be stored in
> fields and passed through locals. Escape/alias analysis for callable values,
> generic `Fn` reassignments across different capture sets, and more exhaustive
> test suites remain future work.

## 1. IR instructions for boxed callables

1. [x] Define `InstKind::MakeCallable { function, capture }` in `crates/zirk-ir/src/ir.rs` to build a two-word callable value from a function reference and a capture block.
2. [x] Define `InstKind::CallCallable { callable, args }` in `crates/zirk-ir/src/ir.rs` to call a boxed callable, loading the function pointer and capture block.
3. [x] Add lowering helpers in `crates/zirk-ir/src/lower.rs` to emit `MakeCallable` and `CallCallable` with the correct argument/return layouts.

## 2. Capture block layout

4. [x] Design the capture-block runtime layout: a heap-allocated object containing the captured values, with a per-closure descriptor computed at lowering time.
5. [x] Implement capture-block allocation in `crates/zirk-runtime/src/` (`zirk_rt_alloc_callable`) and register the C-ABI symbol in `crates/zirk-codegen-llvm/src/runtime.rs`.
6. [x] Implement capture-block deep copying for `.clone()` (`zirk_rt_clone_callable`) when every capture is `Clone`.

## 3. Sema subtyping and checker rules

7. Update `crates/zirk-sema/src/checker.rs` so a `Fn(P...) => R` binding accepts two or more closures with different capture sets at different assignments, as long as the parameter and result types match exactly.
8. Ensure the type of a captured closure is a structural `Fn(...)` and that the capture set is not part of the user-visible type.
9. [x] Reject `.clone()` on a callable when any captured value is not `Clone`, producing a clear diagnostic.

## 4. Lowering of closures and calls

10. [x] In `crates/zirk-ir/src/lower.rs`, lower a captured closure that escapes or is returned to `MakeCallable`, copying captures into the capture block.
11. [x] Update every callable call site in `crates/zirk-ir/src/lower.rs` to emit `CallCallable` when the target type is a structural `Fn(...)` and may be boxed.
12. [x] Ensure named functions and capture-less lambdas are promoted to the two-word `{function pointer, capture-block pointer}` representation at boundaries (parameters, returns, field stores).

## 5. `.clone()` on callables

13. [x] Lower a `.clone()` method call on a callable to `zirk_rt_clone_callable`, producing an independent deep copy of the capture block.
14. [x] Check `Clone` bounds for every captured slot before allowing the call; reject the call with a diagnostic listing the non-`Clone` captures otherwise.
15. [x] Verify that mutating the original captured counter does not affect the cloned callable's counter in runtime tests.

## 6. Escape and alias analysis

16. Update escape and alias analysis in `crates/zirk-sema/src/checker.rs` to treat `Fn(...)` values that may be boxed as heap-escaping when stored or returned.
17. Ensure a captured mutable local used by a callable is not incorrectly considered stack-local after the callable escapes.
18. Add alias-category tests for callable values stored in fields, returned from functions, and passed through parameters.

## 7. Runtime and codegen

19. [x] Register `zirk_rt_alloc_callable` and `zirk_rt_clone_callable` in `crates/zirk-codegen-llvm/src/runtime.rs` and emit the matching LLVM calls in `crates/zirk-codegen-llvm/src/emit.rs`.
20. [x] Emit `MakeCallable` and `CallCallable` in `crates/zirk-codegen-llvm/src/emit.rs`, including the two-word value layout and capture-block pointer.
21. [x] Ensure the shadow-stack and root enumeration paths account for the capture-block pointer inside a boxed callable.

## 8. Tests and validation

22. [x] Add CLI fixtures in `crates/zirk-cli/tests/corpus/` for basic callable assignment and returned closures, stored callables, and `.clone()`.
23. [ ] Add IR lowering tests in `crates/zirk-ir/tests/lowering.rs` for `MakeCallable`, `CallCallable`, and the boxed capture layout.
24. [ ] Add sema tests in `crates/zirk-sema/tests/` for subtyping, escape analysis, and `.clone()` rejection on non-`Clone` captures.
25. [ ] Run `openspec validate phase-4d-callables --strict` after all implementation is in place.
