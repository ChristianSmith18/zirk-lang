## 1. Checker (zirk-sema)

- [x] 1.1 Removed the `not_lowered` gate in `check_conformance` for a `record`/`value class` with non-empty `implements` (`crates/zirk-sema/src/checker.rs`, ~line 1650-1658).
- [x] 1.2 No further checker change was needed for typing the conversion: `is_subclass_of`/`expect_assignable` already treat `Base::Class(id)` uniformly regardless of `ClassKind`, so once the gate was gone the existing subtyping machinery accepted the conversion at every position a class instance already uses (return, assignment, argument-passing).
- [x] 1.3 Checker unit tests added in `crates/zirk-sema/tests/typing.rs` (5 new), replacing the now-obsolete `invalid_record_implements_a_contract_is_not_lowered_yet`.

## 2. IR (zirk-ir)

- [x] 2.1 The `objects` builder (previously an empty placeholder for `Record`/`ValueClass`) now builds a real `ObjectLayout` for a value type once it implements ≥1 contract, reusing the exact same field/methods/contracts construction code a class uses. `lower_box_value` (new): boxing is `Alloc(id)` + one `LoadField`(from the inline `Value`)/`StoreField`(into the fresh `Object`) pair per field — no new `InstKind` needed. Wired into `lower_expr_as` for both the non-nullable and nullable widening paths.
- [x] 2.2 Confirmed the box is read-only: no store/write instruction targets it anywhere in the lowered IR beyond the one-time copy-in at construction.
- [x] 2.3 IR-level tests added to `crates/zirk-ir/tests/lowering.rs`: box lowers to `Alloc`+2×`LoadField`+2×`StoreField`+`Retype`; the box gets a real `ContractTable`; a call through it uses ordinary `CallContract`; a negative check that every `StoreField` in the test program targets only a value just `Alloc`'d in the same block (nothing writes to a box after construction).

**Real generalization gap found and fixed during 2.1 (exactly what this task's own standing instruction asked to verify rather than assume):** a record's own method is compiled with `this` passed *by value* (`IrType::Value`), but `CallContract` always calls the receiver as a *pointer*. Pointing a contract table directly at the record's own method symbol silently miscompiled (garbage field reads, "arithmetic overflow" at runtime) rather than failing to build — a real, latent-until-exercised bug in how contract tables are wired, not something design's own D1/D2 anticipated (both assumed the table could point straight at existing method bodies). Fixed by synthesizing one small unboxing thunk per own-body contract method (`box_thunk_symbol`, `build_box_thunk`): it takes the box's pointer, rebuilds the inline value via `LoadField`+`BuildValue`, and forwards into the real method. A method inherited from a trait's default body needs no thunk — it's already compiled against a generic `this: Contract` pointer. `CallContract` itself was never touched; only which symbol occupies the table slot changed. This preserves design D2's own point (`CallContract`'s dispatch lowering needs zero changes) while correcting an implicit assumption in D1 about what a value type's own method body is compiled to receive.

## 3. Codegen (zirk-codegen-llvm)

- [x] 3.1–3.2 No changes needed in `zirk-codegen-llvm` at all — the box-construction and descriptor-building work all happens in `zirk-ir/src/lower.rs` by reusing the exact same object-layout/descriptor construction a class already goes through, rather than needing a second, codegen-level descriptor builder. This is a cleaner outcome than design anticipated (design expected the reuse to happen at the codegen layer; it happens one layer up, at the layout-construction layer that codegen already consumes uniformly for `Object`).
- [x] 3.3 Confirmed by real compiled-and-run tests (task 4.1-4.3) that `InstKind::CallContract`'s existing codegen lowering needed zero changes to dispatch through a boxed value's descriptor.

## 4. Cross-cutting correctness tests (do not skip)

- [x] 4.1/4.2 **Found blocked for `value class` specifically, not by this change's own design**: `value class`'s compact one-line grammar (`crates/zirk-parser/src/parser.rs::parse_value_class`) hardcodes `implements: Vec::new()` and has no method-body grammar at all — a `value class` cannot satisfy a contract today regardless of this feature, a pre-existing grammar gap this change did not create and is not scoped to fix. Only `record` can exercise this feature today. Delivered instead: `record_contract_dispatch.zrk` (dispatch via variable + parameter, `record` only).
- [x] 4.3 `value_type_contract_polymorphism.zrk`: two different concrete `record` adopters (`Circle`/`Square`) dispatched correctly through the same non-generic function's `Shape`-typed parameter — no `List<T>`/array-literal collection type exists yet in the language, so a loop reassigning one interface-typed variable across iterations is the closest available proxy; it proves the same thing design D1's "Alternative considered" argues static monomorphization cannot: the same static call site, unchanged, dispatches correctly no matter which concrete record is behind it at each call.
- [x] 4.4 `value_type_contract_identity.zrk`: `is` between two separately-boxed equal records is `false` (design D4's own flagged open question resolved: no explicit rejection rule is needed — two independent allocations are simply not the same address); `a is a` is `true`. `invalid_mutating_a_field_through_a_boxed_record_s_contract_type` (checker test): no mutation path exists — a contract declares no fields at all, so an attempted field write through the contract-typed reference fails with `UNKNOWN_MEMBER`, confirming no write path exists by construction (design D3), not by a separate enforcement rule.
- [x] 4.5 Ran `cargo test --workspace` (991 passed); `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 5. Documentation and status sync

- [ ] 5.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — "value-type contract dispatch" delivered for `record`; note the `value class` grammar gap (4.1/4.2's own finding) as the reason it doesn't yet benefit.
- [ ] 5.2 Check `docs/handbook` for any existing value-class/interface teaching material with a stale "not yet implemented" caveat — reconcile.
- [ ] 5.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 5.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 6. OpenSpec close-out

- [ ] 6.1 Run `openspec validate fase-3-value-type-contract-dispatch` before archiving.
- [ ] 6.2 Archive the change once implementation, tests, and documentation sync are complete.
