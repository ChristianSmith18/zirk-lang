# Tasks: fase-3-dispatch-generico

## 1. Semantic support for generic conformance

- [x] 1.1 Record the substituted contract instantiation in `CheckedProgram` conformance data for `class`/`record` `implements Contract<T, ...>`.
- [x] 1.2 Check signature compatibility against the contract's methods after substituting contract type parameters with the `implements` arguments.
- [x] 1.3 Remove the `E0423 NOT_LOWERED` gate for the shapes covered by this change; keep it precise for anything still uncovered.

## 2. IR lowering

- [x] 2.1 Build per-instantiation contract tables in `lower.rs` for generic adopters, substituting contract parameters with the instantiation's arguments.
- [ ] 2.2 Lower trait default bodies under the substitution context.
- [x] 2.3 Emit `CallContract`/`SafeDispatch::Contract` for calls through generic contract references.
- [ ] 2.4 Add a substitution recursion guard producing a controlled diagnostic instead of stack overflow.

## 3. Backend

- [x] 3.1 Verify itable lookup in `emit.rs` covers generic instantiations; extend if the current scheme misses them.
- [ ] 3.2 Confirm vtable offset stability across `Box<Int32>` and `Box<String>` (spec: substitution does not corrupt the vtable).

## 4. Fixtures and status

- [x] 4.1 Extend/add valid corpus fixtures: generic class + generic record implementing `Iterable<T>` (or a user generic contract), call through contract reference, two adopters sharing offsets.
- [ ] 4.2 Add invalid fixtures: missing method under substitution, recursive generic `implements` guard.
- [x] 4.3 Update `docs/init/ZIRK_FEATURE_STATUS.md` generic-contract row.
- [x] 4.4 Run `openspec validate fase-3-dispatch-generico --strict`.

## 5. Verification

- [x] 5.1 `cargo test --workspace` green with `LLVM_SYS_201_PREFIX` set.
- [ ] 5.2 `./scripts/sync-website-content.sh` reviewed for affected public status content (not executed, pending after merge).
