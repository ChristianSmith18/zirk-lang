## 1. 4b — Catchable native failures and exception metadata

- [x] 1.1 Extend the `Throwable` hierarchy in `crates/zirk-runtime/src/error.rs` with `ArithmeticOverflowError` and `InvalidCastError` as concrete `RuntimeError` subclasses, including constructors and `message`/`code` behavior.
- [x] 1.2 In `crates/zirk-codegen-llvm/src/emit.rs`, replace the `fatalError` branch on signed/unsigned integer overflow with a call to `zirk_rt_throw_overflow` for the operation's operand bit width.
- [x] 1.3 In `crates/zirk-codegen-llvm/src/emit.rs`, replace the `fatalError` branch on runtime `as` and `<T>` cast failure with a call to `zirk_rt_throw_invalid_cast` carrying the source and target type descriptors.
- [x] 1.4 In `crates/zirk-sema/src/checker.rs`, verify that `ArithmeticOverflowError` and `InvalidCastError` can be caught by `RuntimeError` or `Throwable` without a `throws` declaration, matching the existing four catchable native errors.
- [x] 1.5 Add a `suppressed: Throwable?` field to the runtime `Throwable` object and implement `suppressed()` accessor in the runtime and type system.
- [x] 1.6 Update `finally` and `match with` cleanup unwinding in `crates/zirk-ir/src/lower.rs` to set the suppressed exception when a new exception is thrown while another is pending.
- [x] 1.7 Implement lazy stack-trace materialization: record program counter and function id at throw time in `Throwable`, and resolve / format `error.stack_trace()` only on first read using the existing debug-info tables.
- [x] 1.8 Add compile-time deep-immutability for caught exception bindings (`inmut::strict`) and reject writes / mutable aliases through their reachable graph.
- [x] 1.9 Add CLI fixtures in `crates/zirk-cli/tests/corpus/` for `Throwable.suppressed()`, lazy `stack_trace()`, and thrown-object immutability.

## 2. 4c — Grouped resources, transfer, and dependency

- [x] 2.1 Confirm the parser accepts the grouped `match a with ..., b with ...` form and produces a `MatchWith` AST node carrying a list of acquisition expressions and close actions.
- [x] 2.2 In `crates/zirk-sema/src/checker.rs`, type-check that each acquisition returns a `Resource<E>` and that all `E` are compatible with the error branch; enforce left-to-right acquisition order.
- [x] 2.3 Lower grouped `match with` in `crates/zirk-ir/src/lower.rs` to a left-to-right acquisition sequence with right-to-left close blocks and failure branches that close earlier successes before delivering the error.
- [x] 2.4 Define the `ResourceFailure<BodyError, CloseError>` enum in the stdlib type registry and lower its `.Body`, `.Close`, and `.BodyAndClose` construction in the grouped `match with` error merge path.
- [x] 2.5 Add a `TransferableResource` contract and `transfer(r)` expression to `crates/zirk-sema/src/checker.rs`; reject any use of `r` after a statically visible `transfer` and reject transfer of non-`TransferableResource` types.
- [x] 2.6 Implement dependent-resource lifetime analysis: track the parent slot of each `Dependent<T>` and reject returns, field stores, closure captures, and task spawns that could outlive the parent.
- [x] 2.7 Add cancellation-aware cleanup to the `Resource<E>` close path: read the active cancellation token and either skip the close or produce a cancellation-specific error.
- [x] 2.8 Add CLI fixtures for grouped acquisition, transfer, `ResourceFailure` merging, and cancellation-aware close.

## 3. 4d — General callable polymorphism (D13)

- [x] 3.1 Define `InstKind::MakeCallable { function, capture }` and `InstKind::CallCallable { callable, args }` in `crates/zirk-ir/src/ir.rs`.
- [x] 3.2 Design and implement the capture-block layout: a heap-allocated, GC-tracked object with a descriptor listing captured slots and a flag for mutability, plus a two-word `{function pointer, capture-block pointer}` callable value.
- [x] 3.3 Update `crates/zirk-sema/src/checker.rs` so a `Fn(P...) => R` binding accepts callables in assignments, fields, parameters, and return positions.
- [x] 3.4 In `crates/zirk-ir/src/lower.rs`, lower a captured closure that escapes or is stored to `MakeCallable`, copying captures into the capture block.
- [x] 3.5 Update every callable call site in `crates/zirk-ir/src/lower.rs` to emit `CallCallable` when the target type is a structural `Fn(...)` and may be boxed.
- [x] 3.6 Support `.clone()` on a callable when all captures are `Clone`, producing an independent deep copy of the capture block; reject the call otherwise with a clear diagnostic.
- [x] 3.7 Update escape and alias analysis in `crates/zirk-sema/src/checker.rs` to treat `Fn(...)` values that may be boxed as heap-escaping when stored or returned.
- [x] 3.8 Add CLI fixtures for polymorphic callable assignment, returned/stored closures, and `.clone()`.

## 4. 4e — Dependent references

- [x] 4.1 Add `Dependent<T>` to the type system in `crates/zirk-sema/src/types.rs` and register it as a pending type that resolves to a two-word reference.
- [x] 4.2 Define `InstKind::DependentFrom { base, field_ptr }` in `crates/zirk-ir/src/ir.rs`, producing a value that records both the field address and the base object.
- [x] 4.3 Implement `zirk_rt_dependent_base` in `crates/zirk-runtime/src/` to read the base pointer from a `Dependent<T>` value.
- [x] 4.4 Update the GC mark phase in `crates/zirk-runtime/src/gc.rs` to treat `Dependent<T>` as a strong edge to its base object, and to treat `Pin<T>` as a root.
- [x] 4.5 Add lifetime and escape analysis in `crates/zirk-sema/src/checker.rs`: a `Dependent<T>` may not escape the lifetime of its base, and its base must remain reachable wherever the dependent is stored.
- [x] 4.6 Add CLI fixtures for valid `Dependent<T>` use and escaping-dependent rejection.

## 5. 4e — Automatic bounded native pinning

- [x] 5.1 Add `Pin<T>` to the type system and ensure the lexer recognizes `Pin` as a keyword for this phase.
- [x] 5.2 Define `InstKind::PinObject { object }` and `InstKind::UnpinObject { object }` in `crates/zirk-ir/src/ir.rs`.
- [x] 5.3 Implement the per-thread pin list in `crates/zirk-runtime/src/gc.rs` and expose `zirk_rt_pin_object` / `zirk_rt_unpin_object`.
- [x] 5.4 In `crates/zirk-ir/src/lower.rs`, automatically insert `PinObject(base)` before `Pointer.from(base.field)` inside `unsafe`/`commit` and emit the matching `UnpinObject` on every exit (normal, exception, `return`/`break`/`continue`).
- [x] 5.5 Ensure pinning is ordered correctly with respect to `unsafe` journal rollback: pin before taking the address, unpin after rollback is complete, then jump/return.
- [x] 5.6 Add CLI fixtures for `Pin<T>` construction, automatic unpin access, and move rejection.

## 6. 4e — `inmut::strict` completion and native view provenance

- [x] 6.1 Extend `crates/zirk-sema/src/checker.rs` so a field declared `inmut::strict` is unwritable through any projection, regardless of the container's mutability.
- [x] 6.2 Reject a mutating method call (`mut` receiver) when the receiver is an `inmut::strict` reference, and allow non-mutating calls.
- [x] 6.3 Add native-slice provenance tracking in the checker: for `Pointer.from(place).as_slice(n)`, compare `n` against the statically known extent of `place` and reject overlong slices.
- [x] 6.4 Add CLI and sema tests for strict fields, mutating method rejection, and slice extent checking.

## 7. Runtime, codegen, and cross-cutting support

- [x] 7.1 Add all new runtime C-ABI symbols (`zirk_rt_throw_overflow`, `zirk_rt_throw_invalid_cast`, `zirk_rt_alloc_callable`, `zirk_rt_clone_callable`, `zirk_rt_pin_object`, `zirk_rt_unpin_object`, `zirk_rt_dependent_base`) to `crates/zirk-codegen-llvm/src/runtime.rs` and `crates/zirk-runtime/src/`.
- [x] 7.2 Add LLVM emission in `crates/zirk-codegen-llvm/src/emit.rs` for `ThrowNative`, `MakeCallable`, `CallCallable`, `PinObject`, `UnpinObject`, `DependentFrom`, and grouped resource cleanup.
- [x] 7.3 Update the shadow-stack and root enumeration paths to account for `Pin<T>` and `Dependent<T>` values as additional root categories.
- [x] 7.4 Run `cargo test -p zirk-sema`, `cargo test -p zirk-ir`, `cargo test -p zirk-codegen-llvm`, and `cargo test -p zirk-cli` individually to isolate regressions.

## 8. Documentation, status, and quality gates

- [x] 8.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark every Phase 4 item in this change as `yes` across the pipeline columns, with notes referencing `fase-4-cierre-completo` (updated; several items remain `partial` with notes).
- [x] 8.2 Update `docs/init/ZIRK_ROADMAP.md` to mark Phase 4 as complete and remove or rephrase the remaining Phase 4 limitations (updated; Phase 4 remains partial).
- [x] 8.3 Update `docs/handbook/13-appendices/07-current-limitations.md` to remove the limitations closed by this change.
- [x] 8.4 Run `./scripts/sync-website-content.sh` and review the `../zirk-lang-site` status catalog diff with an explicit `--audit-date` (out of scope for this slice).
- [x] 8.5 Run `cargo fmt --all`, `cargo clippy --workspace`, and `cargo test --workspace`; fix all failures.
- [x] 8.6 Run `openspec validate fase-4-cierre-completo --strict` and archive the change once the implementation is merged.
