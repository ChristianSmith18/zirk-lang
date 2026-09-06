# Tasks: fase-3-cierre-contratos

## 1. Qualified trait selection `TraitName.super.method()`

- [ ] 1.1 Parse `TraitName.super.method(args)` in expression position (new AST node or extension of the existing `Super` node) per `zirk-classes` "Explicit overriding and super dispatch".
- [ ] 1.2 Sema: resolve `TraitName` against the adopter's implemented traits; error when the trait is not adopted or declares no such default.
- [ ] 1.3 Lower to a direct (non-virtual) call of the selected trait default body with `this` as receiver; codegen end-to-end.
- [ ] 1.4 Fixtures: conflict resolved by explicit selection (valid), trait not adopted / no such method (invalid).

## 2. Contract composition

- [ ] 2.1 Parse `implements` on `interface` and `trait` declarations.
- [ ] 2.2 Build the contract inheritance DAG at declaration collection; reject cycles and incompatible same-name signatures with diagnostics.
- [ ] 2.3 Transitive conformance: an adopter of `interface B implements A` must satisfy A's requirements (declared or via trait defaults).
- [ ] 2.4 Fixtures: diamond of interfaces (valid), cycle (invalid), incompatible signatures (invalid).

## 3. `as?` nullable cast

- [ ] 3.1 Parse `expr as? Type`; AST node for nullable cast.
- [ ] 3.2 Sema: result type `T?`; unrelated casts remain compile-time errors.
- [ ] 3.3 Lower to the existing checked-cast descriptor comparison, producing `null` on mismatch instead of a runtime failure.
- [ ] 3.4 Fixtures: successful/failed downcast to null, unrelated cast rejected.

## 4. Type-parameter defaults

- [ ] 4.1 Parse `<T = Int32>` (trailing only); AST field on `TypeParam`.
- [ ] 4.2 Sema: use the default when the argument is omitted; verify the default against `from` constraints at declaration and use site.
- [ ] 4.3 Fixtures: omitted argument uses default, default violating constraint rejected, non-trailing default rejected.

## 5. Comparison operator contracts

- [ ] 5.1 Confirm reserved-method names against `lower.rs`'s operator table and spec wording; map `<`, `<=`, `>`, `>=` to them.
- [ ] 5.2 Dispatch comparisons on user types through contract calls, mirroring `+`/`==`.
- [ ] 5.3 Fixtures: user type implementing comparison contract (valid), operator on non-implementing type (invalid, names the missing contract).

## 6. Status and verification

- [ ] 6.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` rows for the five items.
- [ ] 6.2 `cargo test --workspace` green with `LLVM_SYS_201_PREFIX` set.
- [ ] 6.3 `openspec validate fase-3-cierre-contratos --strict` clean.
- [ ] 6.4 Note `./scripts/sync-website-content.sh` as pending post-merge (do not run it).
