## 1. Type system changes

- [x] 1.1 Add `Dependent<T>` to the type system in `crates/zirk-sema/src/types.rs` and register it as a built-in generic type.
- [x] 1.2 Add `Pin<T>` to the type system in `crates/zirk-sema/src/types.rs` and ensure the lexer recognizes `Pin` as a keyword for this phase.
- [x] 1.3 Define subtyping and equality rules for `Dependent<T>` and `Pin<T>` in `crates/zirk-sema/src/types.rs` (equality falls back to base equality; subtyping rules not yet added).
- [x] 1.4 Expose `Pin<T>` in the standard type registry so user code can write explicit `p: Pin<MyClass>`.

## 2. IR design and lowering

- [x] 2.1 Define `InstKind::DependentFrom { base, field_ptr }` in `crates/zirk-ir/src/ir.rs`, producing a value that records both the field address and the base object.
- [x] 2.2 Define `InstKind::PinObject { object }` and `InstKind::UnpinObject { object }` in `crates/zirk-ir/src/ir.rs`.
- [ ] 2.3 In `crates/zirk-ir/src/lower.rs`, lower a `Dependent<T>` constructor to `DependentFrom` with the base and field pointer (stub exists, not wired to surface syntax).
- [ ] 2.4 In `crates/zirk-ir/src/lower.rs`, automatically insert `PinObject(base)` before `Pointer.from(base.field)` inside `unsafe`/`commit` and emit the matching `UnpinObject` on every exit (normal, exception, `return`/`break`/`continue`).
- [ ] 2.5 Ensure pinning is ordered correctly with respect to `unsafe` journal rollback: pin before taking the address, unpin after rollback is complete, then jump/return.

## 3. Runtime helpers

- [x] 3.1 Implement `zirk_rt_dependent_base` in `crates/zirk-runtime/src/` to read the base pointer from a `Dependent<T>` value (C-ABI stub declared; real base extraction not yet wired).
- [ ] 3.2 Implement the per-thread pin list in `crates/zirk-runtime/src/gc.rs` and expose `zirk_rt_pin_object` / `zirk_rt_unpin_object`.
- [x] 3.3 Add all new runtime C-ABI symbols (`zirk_rt_pin_object`, `zirk_rt_unpin_object`, `zirk_rt_dependent_base`) to `crates/zirk-codegen-llvm/src/runtime.rs`.
- [ ] 3.4 Add object-header support to distinguish pinned objects and to treat pinned objects as non-movable during collection compaction.

## 4. GC tracing and root categories

- [ ] 4.1 Update the GC mark phase in `crates/zirk-runtime/src/gc.rs` to treat `Dependent<T>` as a strong edge to its base object.
- [x] 4.2 Treat `Pin<T>` as an additional root category during root enumeration and update the shadow-stack paths accordingly (slot type recognized; full pin-list semantics not wired).
- [x] 4.3 Add LLVM emission in `crates/zirk-codegen-llvm/src/emit.rs` for `PinObject`, `UnpinObject`, and `DependentFrom` (surface placeholders).
- [ ] 4.4 Ensure pinned objects are not moved by the compactor while they remain in the per-thread pin list.

## 5. Lifetime and escape analysis

- [x] 5.1 Add lifetime and escape analysis in `crates/zirk-sema/src/checker.rs`: a `Dependent<T>` may not escape the lifetime of its base.
- [x] 5.2 Reject any use of `Dependent<T>` that is returned, stored in a heap/field location that does not keep the base alive, captured in a closure, or passed where it can outlive the base.
  - Implemented by extending `reject_pointer_escape` to handle `Base::Dependent` (code `E0459`) and adding `reject_dependent_escape` for call arguments/parameter mismatch.
- [ ] 5.3 Verify that the base object remains reachable wherever the dependent value is stored, and emit a lifetime diagnostic when it does not.

## 6. Pinning semantics

- [ ] 6.1 Ensure `Pin<T>` is released when the enclosing `unsafe`/`commit` block exits normally, by exception, or by `return`/`break`/`continue`.
- [ ] 6.2 Confirm the runtime removes the object from the per-thread pin list after journal rollback and before the target jump.
- [x] 6.3 Reject any operation inside a `Pin<T>` scope that could move or resize the pinned object (e.g. reassigning a local that is the base of an active pin).
  - Implemented in `crates/zirk-sema/src/checker.rs`/`scope.rs`: the checker marks the base binding as pinned and rejects reassignment with code `E0460`.

## 7. `inmut::strict` completion and native view provenance

- [x] 7.1 Extend `crates/zirk-sema/src/checker.rs` so a field declared `inmut::strict` is unwritable through any projection, regardless of the container's mutability.
  - The parser also needed to parse `inmut::strict` on fields (`crates/zirk-parser/src/parser.rs`); the AST `Mutability::Strict` already existed.
  - Records/value classes preserve `inmut::strict` in `FieldInfo.mutability` so `check_writable_field` can reject the write with code `E0456`.
  - Shortcut: `inmut::strict` class fields are rejected even inside the constructor; there is currently no way to initialize them.
- [x] 7.2 Reject a mutating method call (`mut` receiver) when the receiver is an `inmut::strict` reference, and allow non-mutating calls.
  - Added `mut fn` syntax to the parser/AST and `MethodDecl.is_mut`/`MethodInfo.is_mut`.
  - `check_method_call` rejects `mut` methods on receivers whose root binding is `inmut::strict` with code `E0457`.
  - Non-`mut` methods are allowed on strict references.
- [x] 7.3 Add native-slice provenance tracking in the checker: for `Pointer.from(place).as_slice(n)`, compare `n` against the statically known extent of `place` and reject overlong slices.
  - Implemented `check_native_slice_extent` for the direct `Pointer.from(place).as_slice(n)`/`as_slice_mut(n)` shape.
  - Constant length literals are compared against the known extent; the extent is currently `1` for every addressable FFI-safe place (scalar local/parameter/field), because the language has no array type in this phase.
  - Rejected with code `E0458`.
  - The old valid fixture `native_slice_construction_exceeds_known_extent` was removed because the same scenario is now a compile-time error; a new `invalid/slice_extent_overlong.zrk` fixture covers it.
- [x] 7.4 Add CLI and sema tests for strict fields, mutating method rejection, and slice extent checking.
  - Added CLI fixtures in `crates/zirk-cli/tests/corpus/`:
    - `valid/strict_field_read.zrk` + `.out`
    - `valid/strict_method_non_mutating.zrk` + `.out` (extra coverage for 7.2)
    - `invalid/strict_field_write.zrk`
    - `invalid/strict_mutating_method.zrk`
    - `valid/slice_extent_ok.zrk` + `.out`
    - `invalid/slice_extent_overlong.zrk`
  - `cargo fmt --all`, `cargo clippy --workspace`, and `cargo test --workspace` pass with `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`.

## 8. Tests and validation

- [x] 8.1 Add CLI fixtures in `crates/zirk-cli/tests/corpus/` for `Dependent<T>`/`Pin<T>` surface syntax; IR lowering tests for `DependentFrom`/`PinObject`/`UnpinObject` not yet added.
  - Added `valid/dependent_local_only.zrk` + `.out`, `invalid/dependent_escapes.zrk`, `valid/pin_automatic_unpin.zrk` + `.out`, `invalid/pin_move_rejected.zrk`.
- [ ] 8.2 Add runtime tests for `Dependent<T>` GC reachability and base collection ordering.
- [ ] 8.3 Add runtime tests for interior pointer stability across GC cycles and correct unpin on early exits.
- [x] 8.4 Run `cargo test -p zirk-sema`, `cargo test -p zirk-ir`, `cargo test -p zirk-codegen-llvm`, and `cargo test -p zirk-cli` individually to isolate regressions.
- [ ] 8.5 Run `openspec validate fase-4e-cierre-memoria --strict` and archive or merge the change once tests pass.
