## 1. Lexer and keywords

- [x] 1.1 Un-gate `unsafe`, `Pointer` from `Phase::FOUR` in `crates/zirk-lexer/src/token.rs`'s `phase()` (they are already reserved keywords, just phase-blocked — confirm `unsafe`'s current gating and remove it; `Pointer` is a type name resolved in `zirk-sema`, not a lexer keyword, so check `crates/zirk-sema/src/types.rs`'s `PHASE_4` pending-type list instead and remove `Pointer` from it once the type is real).
- [x] 1.2 Add `commit` and `extern` as new reserved keywords (not phase-gated — both ship in this change). Add lexer tests mirroring the existing keyword-phase test pattern (`crates/zirk-lexer/src/token.rs`'s own `#[cfg(test)]` module).

## 2. AST

- [x] 2.1 Add `Stmt::Unsafe(Block)` or equivalent (design: does `unsafe {}` need its own statement node, or can it reuse `Block` with a flag? Prefer a dedicated node — `unsafe` changes checker/IR behavior for everything inside it, unlike an ordinary nested block).
- [x] 2.2 Add `Stmt::Commit(Block)` (or the equivalent expression-position node if `unsafe {}`/`commit {}` are expressions with a value in this grammar — check whether `MEMORY_AND_UNSAFE_SEMANTICS.md`'s own example, `mut result = unsafe { ... };`, requires unsafe/commit to be expression-valued like `if`/`match` already are in this language; if so, model them alongside those rather than as bare statements).
  - **Decision taken**: yes, expression-valued, following the `if`/`match` precedent exactly. `UnsafeBlock`/`CommitBlock` are single shared structs, each used by both a `Stmt::` variant (effect position) and an `Expr::` variant (value position, `Box`ed) — the same shape `IfStmt` already has between `Stmt::If`/`Expr::If`.
- [x] 2.3 Add an `unsafe` modifier to the function declaration AST node (`unsafe fn`).
- [x] 2.4 Add a new top-level item AST node for `extern "C" fn name(params): ReturnType;` — no body, records the convention literal, parameters, and return type.
- [x] 2.5 Add `TypeRef` support for `Pointer<T>` (a single-type-argument type reference, reusing whatever generic-type-reference parsing already exists for user generics if convenient, per design D1 — the AST-level representation can stay uniform even though the checker treats `Pointer<T>` as its own `Base` variant, not a generic instantiation).

## 3. Parser

- [x] 3.1 Parse `unsafe fn` as a function modifier, alongside existing modifiers.
- [x] 3.2 Parse `unsafe { ... }` (as an expression or statement per task 2.2's resolution).
- [x] 3.3 Parse `commit { ... }`, valid syntactically anywhere `unsafe {}` is (the *semantic* "only inside unsafe" rule is the checker's job, task 5.3 — the grammar itself should still parse `commit {}` outside unsafe so the checker can produce the "Commit appears outside unsafe" diagnostic the grammar spec's own scenario already requires, rather than a raw parse error).
- [x] 3.4 Parse `extern "C" fn name(params): ReturnType;` as a top-level item. Reject a body (`{ ... }` after the signature) with a dedicated diagnostic, not a generic parse error (spec scenario "A body is rejected"). Reject a convention literal other than `"C"` with a dedicated diagnostic naming it (spec scenario "Unsupported convention is rejected").
- [x] 3.5 Parse `Pointer<T>` as a type reference wherever other type references are accepted (parameter, return, field, local annotation, generic argument).
- [x] 3.6 Parser unit tests: `unsafe fn`, `unsafe {}`, `commit {}` (both nested-in-unsafe and bare, since the grammar accepts the bare form per 3.3), `extern "C" fn` (valid, missing-body-rejected doesn't apply at parse time per 3.4's design — a body IS parsed and rejected with a dedicated message, not omitted from grammar), unsupported convention literal, `Pointer<T>` in each type position.

## 4. Checker — types

- [x] 4.1 Add `Base::Pointer(Box<Type>)` (or the interned equivalent matching this codebase's existing `Type`/`Base` representation) to `crates/zirk-sema/src/types.rs` (design D1).
- [x] 4.2 Add `Byte` as a resolved alias of `UInt8`, alongside the existing `Int`/`Integer` → `Int32` alias mechanism (`crates/zirk-sema/src/types.rs`, the `"Int32" | "Int" | "Integer" => ...` pattern).
- [x] 4.3 Implement `fn is_ffi_safe(ty: Type) -> bool` (design D2): `Boolean`, every fixed-width `Int`/`UInt`, `Float32`/`Float64`, and `Base::Pointer(inner)` where `inner` is itself FFI-safe (recursively); `Void` allowed only as a return type, checked at its own call sites, not inside this predicate.
- [x] 4.4 Resolve `Pointer<T>` type references: reject `T` that is not FFI-safe (spec scenario "Disallowed element type"), pointing at `T`'s own span.
- [x] 4.5 Resolve `extern "C" fn` signatures: reject any parameter whose type is not FFI-safe, and a return type that is not FFI-safe-or-`Void` (spec scenario "Disallowed extern parameter type").

## 5. Checker — context tracking (design D3)

- [x] 5.1 Add `unsafe_depth: u32` and `commit_depth: u32` to the checker's state, incremented/decremented around `check_block` for `unsafe {}`/`commit {}` respectively — mirror the existing `loop_depth` pattern exactly.
- [x] 5.2 `commit {}` requires `unsafe_depth > 0` at the point it opens, checked before incrementing `commit_depth` (spec scenario "Commit appears outside unsafe", already required by the existing "Contextual safety restrictions" grammar requirement this change delivers).
- [x] 5.3 Each `Pointer<T>` operation requiring unsafe (`Pointer.from`, `.read()`, `.write()`, `.offset()`, `.offset_bytes()`, `.cast<U>()` — NOT `.is_null`, which is exempt per the type-system spec delta) checks `unsafe_depth > 0` (spec scenario "Pointer operation outside unsafe").
- [x] 5.4 An `extern "C" fn` call site checks both `unsafe_depth > 0` and `commit_depth > 0` (spec scenario "Extern call outside commit").
- [x] 5.5 Checker unit tests for each context rule: pointer op outside unsafe rejected, pointer op inside unsafe accepted, commit outside unsafe rejected, commit inside unsafe accepted, extern call inside unsafe-but-not-commit rejected, extern call inside both accepted.

## 6. Checker — `Pointer<T>` operations and escape rule (design D4)

- [x] 6.1 Type `Pointer.from(place)`: `place` must be an addressable local, parameter, or field (an lvalue the checker already knows how to resolve to a slot/field — reuse whatever `check_assign_target`/place-resolution machinery already exists); produces `Pointer<T>` where `T` is `place`'s own type (which must itself be FFI-safe, task 4.4 catches this).
- [x] 6.2 Type `.is_null` (`Boolean`, no unsafe required), `.read()` (`T`), `.write(value: T)` (`Void`), `.offset(n: Int/UInt)` (`Pointer<T>`), `.offset_bytes(n: Int/UInt)` (`Pointer<T>`), `.cast<U>()` (`Pointer<U>`, `U` checked FFI-safe per 4.3).
  - **Decision taken**: `.cast<U>()` is spelled `pointer as Pointer<U>` using the language's existing `as`/`<Type>` cast expression, not a new `.cast<U>()` call syntax — `zirk-ast::CallExpr` has no explicit-generic-argument call form anywhere in the grammar (confirmed by search: `resolve_class_reference`'s own doc comment notes generic *construction* has none either), so inventing one for this single case would be new grammar surface the rest of the language does not have. `Pointer<T>` is already an ordinary `TypeRef`, so `ptr as Pointer<U>` reuses `CastExpr` unmodified: `Checker::casts_are_related`/`cast_is_directly_lowerable` gained a `(Base::Pointer, Base::Pointer)` case, and `check_cast` requires `unsafe` when both sides are pointers.
- [x] 6.3 Implement the blanket escape rule (design D4): reject a `return` expression, a field-assignment source, or a closure capture whose static type is `Pointer<T>` for any `T` — reuse whichever single check function makes sense to call from all three sites, rather than three separate ad hoc checks.
- [x] 6.4 Checker unit tests: `Pointer.from` on a local/parameter/field each typed correctly; each operation's result type; escape rejected from `return`, from a field write, and from closure capture; escape NOT rejected for ordinary local use (read/write inside the same function, no escape).

## 7. Checker — journal/commit semantics validation (design D5/D6)

- [x] 7.1 No new checker rule is strictly required for journaling itself (it is a lowering/runtime concern, not a type rule) — but confirm the checker still runs its existing resource-cleanup-on-unwind rules (`fase-4c-recursos`) correctly when they occur inside an `unsafe {}`/`commit {}` block, since design D6 requires reusing that mechanism, not bypassing it. Add a test confirming a `Resource<E>`'s `close()` still runs on unwind through an unsafe block.
  - **Scope note**: the journal itself (D5/D6, tasks 8.3/8.4/10) is cut from this pass — see the note at the top of section 8. `unsafe {}`/`commit {}` lower as ordinary blocks, so the existing resource-cleanup-on-unwind path is untouched by construction (nothing in this slice changed how `try`/`finally`/`Resource<E>` lower); no dedicated test was added since there is no unsafe-specific interaction yet to regress.

## 8. IR lowering

**Scope cut taken mid-implementation** (per design.md's own "Risks/Trade-offs" and the session's explicit permission to split D1-D4+D7-D8 from D5-D6 if the full slice did not fit): the transactional journal (design D5/D6 — `zirk_rt_journal_begin/record/commit/rollback`, rollback-on-exception) is **not implemented**. `unsafe {}`/`commit {}` lower as ordinary blocks (`FunctionLowering::lower_stmt`'s own comment on this), which the checker's context tracking (section 5) still gates correctly — a program is rejected/accepted by the same rules either way — but no write inside `unsafe {}` is journaled or rolled back on failure yet. Tasks 8.3/8.4/10.x below are left unchecked, with this note as the record of why. Everything else (D1-D4, D7-D8: the pointer core and `extern`) is implemented and tested end-to-end.

- [x] 8.1 Add `IrType::Pointer(Box<IrType>)` to `crates/zirk-ir/src/ir.rs`.
  - **Representation note**: `IrType::Pointer(u32)`, not `Box<IrType>` — `IrType` derives `Copy`, and a `Box` field would break that everywhere the type is passed around. The pointee is interned in a new `Module::pointer_types: Vec<IrType>` table instead, populated 1:1 in the same order as the checker's own `CheckedProgram::pointer_types`, so a `Base::Pointer(id)`/`IrType::Pointer(id)` pair always names the same id without re-interning (mirrors how `Closure`/`Object`/`Value`/`Enum` already work).
- [x] 8.2 Add IR instructions for pointer operations: address-of a slot/field (`Pointer.from`, design D8), read, write, offset (element units), offset-bytes, cast, is-null. Match this codebase's existing instruction-naming conventions (see `InstKind`'s existing variants for style).
  - Added as `InstKind::PointerFromSlot`, `PointerFromField`, `PointerRead`, `PointerWrite`, `PointerOffset`, `PointerOffsetBytes`, `PointerCast`, `PointerIsNull`. `PointerFromField` only supports an `Object`-kind receiver (a record/value class field has no address of its own to take without a separate "where does this value live" decision, which this slice does not need — the FFI-safe element restriction keeps this narrow in practice; documented at the codegen site too).
- [ ] 8.3 ~~Lower `unsafe { ... }` (design D5)~~ — **cut, see the scope note above.** Lowers as an ordinary block instead.
- [ ] 8.4 ~~Lower `commit { ... }` (design D5)~~ — **cut, see the scope note above.** Lowers as an ordinary block instead.
- [x] 8.5 Lower `extern "C" fn` declarations as external function declarations (design D7) — no body to lower, only a declaration codegen picks up (task 9.1). Collected into a new `Module::externs: Vec<ExternFn>` in `zirk_ir::lower::lower`.
- [x] 8.6 Lower a call to an `extern "C" fn` through whatever `Call` instruction already exists for ordinary function calls (design D7) — confirm it works unmodified for a callee with no Zirk-authored body.
  - `FunctionLowering::signature_return`/`lower_args` gained an extern fallback (checked `checked.externs` when `checked.functions` has no entry) — `InstKind::Call { callee, args }` itself needed no change, exactly as D7 predicted.
- [x] 8.7 IR-level tests: covered end-to-end (task 12), following the `fase-4d-declaraciones-multiples` precedent of not adding a separate IR-snapshot layer — `crates/zirk-ir/tests/verification.rs` also gained the two struct-literal updates the new `Module` fields required, keeping its existing tests green.

## 9. Codegen (zirk-codegen-llvm)

- [x] 9.1 Emit `extern "C" fn` declarations as LLVM external function declarations with the C calling convention, no body (design D7, spec scenario "Declaration becomes an external symbol"). `declare_extern_fn` in `emit.rs`, under the real (unprefixed) symbol name — unlike an ordinary Zirk function, which gets `FUNCTION_PREFIX` and a `define`.
- [x] 9.2 Emit pointer read/write as LLVM `load`/`store` through the pointer operand, typed by `T` (design D8, spec scenario "Pointer write lowers to a store").
- [x] 9.3 Emit `.offset(n)` as `getelementptr` in units of `T` (design D8, spec scenario "Element offset lowers to `getelementptr`"), and `.offset_bytes(n)` as `getelementptr` over an `i8`-typed view of the same pointer.
- [x] 9.4 Emit `.cast<U>()` as an LLVM pointer bitcast.
  - Under LLVM's opaque-pointer model a `ptr` value carries no pointee type, so the "bitcast" is the identity function on the operand — only its declared `IrType` differs from that point on.
- [x] 9.5 Emit `Pointer.from(place)` as the existing address (`alloca`/GEP) already computed for that slot/field — confirm no new storage is allocated, only an existing address exposed.

## 10. Runtime (zirk-runtime)

**Not implemented — cut along with D5/D6, see section 8's scope note.** No `unsafe_journal.rs` module, no `zirk_rt_journal_*` symbols. Tasks 10.1-10.5 are left unchecked; this is the natural resumption point for a follow-up session, together with IR tasks 8.3/8.4.

- [ ] 10.1 (not started)
- [ ] 10.2 (not started)
- [ ] 10.3 (not started)
- [ ] 10.4 (not started)
- [ ] 10.5 (not started)

## 11. Diagnostics

- [x] 11.1 New diagnostic codes (next free `E0...`, following existing numbering in `crates/zirk-sema`) for: pointer operation outside unsafe; `Pointer<T>` escape (return/field/capture — one code or three, whichever matches this codebase's existing granularity for related-but-distinct diagnostics); disallowed `Pointer<T>`/`extern` element type; extern call missing unsafe and/or commit; extern declaration with a body; extern declaration with an unsupported convention.
  - **Decision taken**: one shared code for all three escape forms (`E0448 POINTER_ESCAPES`) rather than three — matches this codebase's own precedent (`E0417 CAPTURED_MUTATION` is one code covering more than one syntactic shape). Added: `E0444 NOT_FFI_SAFE`, `E0445 POINTER_OP_OUTSIDE_UNSAFE`, `E0446 COMMIT_OUTSIDE_UNSAFE`, `E0447 EXTERN_CALL_OUTSIDE_UNSAFE_COMMIT`, `E0448 POINTER_ESCAPES` (`zirk-sema`); `E0314 EXTERN_HAS_BODY`, `E0315 EXTERN_BAD_CONVENTION` (`zirk-parser`).
- [x] 11.2 Diagnostic golden/snapshot tests for each, consistent with existing `E0` diagnostic tests in this repo — this codebase's own convention is asserting `output.contains(codes::X.as_str())` against a rendered diagnostic (not literal golden files); every new code above has at least one triggering test in `crates/zirk-sema/tests/typing.rs` and `crates/zirk-parser/tests/grammar.rs`.

## 12. Cross-cutting tests

- [x] 12.1 End-to-end `.zrk` fixtures in `crates/zirk-cli/tests/corpus/valid/`: `pointer_read_write.zrk` (`Pointer.from` on a local, `.read()`/`.write()` inside `unsafe {}`), `pointer_offset_cast.zrk` (`.offset(0)`, `ptr as Pointer<U>` reinterpret cast, `.is_null`), `extern_call.zrk` (declares and calls libc's `abs` inside `unsafe { commit { ... } }` — confirmed resolvable: `zirk-runtime`'s own link already pulls in libc transitively, so no new link-setup change was needed).
- [ ] 12.2 ~~End-to-end fixture demonstrating rollback~~ — **cut along with D5/D6.** Nothing rolls back yet, so this fixture cannot be written meaningfully; resume together with task 8.3/10.
- [ ] 12.3 ~~End-to-end fixture demonstrating durable commit~~ — **cut along with D5/D6**, same reason as 12.2.
- [x] 12.4 End-to-end `.zrk` fixtures in `crates/zirk-cli/tests/corpus/invalid/`: `pointer_op_outside_unsafe.zrk`, `pointer_escapes_return.zrk`, `pointer_escapes_field.zrk`, `pointer_escapes_capture.zrk`, `extern_call_missing_commit.zrk`, `extern_declaration_with_body.zrk`, `extern_unsupported_convention.zrk`, `pointer_disallowed_element_type.zrk`.
- [x] 12.5 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` (881 passed, 27 suites), `cargo clippy --workspace --all-targets` (0 issues), `cargo fmt --check` (clean).

## 13. Documentation and status sync

- [x] 13.1 Update `docs/init/ZIRK_ROADMAP.md` Phase 4e bullet "Implement `unsafe {}`, `Pointer<T>`, native slices, and compiler-enforced memory-safety boundaries" to record the pointer-core + `extern` slice delivered, `NativeSlice<T>`/volatile/native-union/weak-atomics still pending. (Rewrote the whole Phase 4e section with a status line and per-bullet `[x]`/`[ ]` + notes, matching Phase 4d's own format — it had none before.)
- [x] 13.2 Update `docs/handbook/13-appendices/07-current-limitations.md`'s memory/unsafe bullet (already touched by `fase-4e-inmut-strict-proyeccion`) to reflect this slice.
- [x] 13.3 Update `docs/handbook/11-reference/12-feature-status.md`'s "memory, native views and transactional unsafe" row.
- [x] 13.4 Check `docs/handbook` for any existing `unsafe`/`Pointer<T>`/`extern` teaching material that currently carries a "not yet implemented" caveat this change should remove, or that documents syntax this change's actual implementation ended up differing from — reconcile either direction. (Checked `05-native-and-low-level/{01-c-abi,02-importing-c}.md`, `02-handbook/17-memory-and-safety/{05-pointers,07-unsafe-blocks,12-transactional-unsafe-and-commit}.md` — all conceptual/prose, no literal `extern`/`Pointer<T>` code syntax that conflicts with this change's actual grammar, and none carry a stale caveat. Nothing to reconcile.)
- [ ] 13.5 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using today's date if project-status evidence changed).
- [ ] 13.6 Report both the zirk-lang and zirk-lang-site revisions used so the synchronization is auditable.

## 14. OpenSpec close-out

- [ ] 14.1 Run `openspec validate fase-4e-unsafe-pointer-extern` before archiving.
- [ ] 14.2 Archive the change once implementation, tests, and documentation sync are complete.
