## 1. Checker (zirk-sema)

- [x] 1.1 Remove the `not_lowered` gate for `record`/`value class` in `reject_unstructured_comparison` (`crates/zirk-sema/src/checker.rs`, ~line 6528-6548). Replaced with a recursive per-field support check (`structural_equality_unsupported_field`) so a residual unsupported field type (e.g. `T?`) still gets the diagnostic, scoped to just that field.
- [x] 1.2 Confirm the exact current rule for a bare `class`-typed field's own `==` (identity via `is`, or the referenced class's own `_equals` if declared) by reading the actual current checker behavior (design D2 — do not restate an assumed rule, verify it). Confirmed: `operator_method_of`/`reject_unstructured_comparison` only ever dispatch to `_equals` when it exists; a `class` field with no `_equals` has no checker-level restriction at the nested-field position (only a bare top-level `class == class` errors), so D2's "identity via `is` otherwise" is what `lower_field_equality` implements for that case.
- [x] 1.3 Checker unit tests added in `crates/zirk-sema/tests/typing.rs`: `valid_record_equality_is_derived_without_a_reserved_method` (updated to assert full acceptance, not just absence of `TYPE_MISMATCH`), `valid_value_class_equality_is_derived_without_a_reserved_method`, `valid_record_equality_over_a_nested_record_field`, `valid_record_equality_over_a_class_field_with_its_own_equals`, `valid_record_equality_over_a_class_field_without_equals_uses_identity`, `invalid_record_equality_over_an_unsupported_field_type_is_not_lowered`.

## 2. IR (zirk-ir)

- [x] 2.1 Lower `Eq`/`NotEq` on a `record`/`value class` operand (with no user `_equals`) to a conjunction of per-field comparisons (design D1) in `crates/zirk-ir/src/lower.rs` (`lower_structural_equality`/`lower_field_conjunction`/`lower_field_equality`). Confirmed the actual short-circuit shape via `lower_short_circuit` (declare a boolean slot, branch/store/jump between blocks) and reused that exact block pattern, generalized from two operands to however many fields the type declares — `&&`/`||` themselves are untouched, no new conjunction mechanism.
- [x] 2.2 Recurses correctly into a nested `record`/`value class` field: `lower_field_equality` matches `IrType::Value(id)` and calls `lower_field_conjunction` again for that nested layout, rather than flattening its fields into the parent's list.
- [x] 2.3 `!=` reuses the same generated comparison, negated: `lower_structural_equality` calls `lower_field_conjunction` once and wraps the result in `InstKind::Unary { op: UnaryOp::Not, .. }` for `NotEq`, the same split every other `Eq`/`NotEq` arm in `lower.rs` already uses (confirmed at the `_equals`-dispatch arm and the generic `Binary` arm).
- [x] 2.4 Covered end-to-end rather than as separate IR-only tests, since the real compiled-and-run corpus fixture (task 4.*) already exercises the flat, nested, and short-circuit cases against actual codegen output — a pure IR-shape test would duplicate that without adding confidence codegen doesn't already need to earn on its own (task 3.1).

## 3. Codegen (zirk-codegen-llvm) — only if lowering to existing IR shapes isn't already sufficient

- [x] 3.1 Lowering emits only pre-existing IR instruction kinds (`InstKind::LoadField`, `InstKind::Binary { op: Eq | Identical }`, `InstKind::Call`, `InstKind::Unary { op: Not }`, plus the same `Store`/`Load`/`Branch`/`Jump` shape `&&` already uses) — no new codegen work. Verified with a real compiled-and-run fixture (`crates/zirk-cli/tests/corpus/valid/structural_equality.zrk`), not by inspection alone. One pre-existing codegen constraint drove a checker-side scope decision: `emit_binary` for a pointer-shaped `Eq`/`NotEq` operand unconditionally calls `compare_strings` (the runtime `String` equality path) for anything that isn't `Identical`, so a bare `Binary { op: Eq }` cannot be emitted for an `Object`-typed field (would wrongly call the string-equality runtime function on a class pointer) or for a `Nullable`/algebraic-`Enum` field (struct-shaped, not handled by `emit_binary`'s `Eq` path at all, only by its `Identical`/`is` path) — this is why `Object` fields get their own `_equals`-or-`Identical` dispatch in `lower_field_equality`, and why the checker rejects (via 1.1's now-scoped diagnostic) a field whose type doesn't fit one of the three shapes this lowering actually builds.

## 4. Cross-cutting correctness tests (do not skip)

- [x] 4.1 End-to-end `.zrk` fixture (`structural_equality.zrk`): `Point` records — `a == b` (identical fields) is `true`, `a == c` (one field differs) is `false`.
- [x] 4.2 End-to-end `.zrk` fixture: `Money` value class — `m1 == m2` (identical) is `true`, `m1 == m3` (currency differs) is `false`.
- [x] 4.3 End-to-end `.zrk` fixture: `Line { start: Point; end: Point; }` — `l1 == l2` (identical nested `Point`s) is `true`, `l1 == l3` (nested `end` differs) is `false`.
- [x] 4.4 End-to-end `.zrk` fixture: `a != b` is `false` for an equal pair, `a != c` is `true` for an unequal pair.
- [x] 4.5 Confirmed short-circuiting for real, end-to-end: `Timed { at: Int32; tick: Tick; }` where `Tick._equals` prints `"tick compared"` when invoked — comparing two `Timed` values differing only in `at` (the first field) never prints that line, while comparing two differing only in `tick` (with equal `at`) does. This observes evaluation order directly through a real side effect, not just an IR-level assertion.
- [x] 4.6 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` (972 passed), `cargo clippy --workspace --all-targets` (clean), `cargo fmt --check` (clean, after `cargo fmt`).

## 5. Documentation and status sync

- [ ] 5.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — drop "derived structural equality" from the "Several Phase 3 constructs..." bullet/row once delivered.
- [ ] 5.2 Check `docs/handbook/02-handbook/05-operators-and-expressions/03-structural-equality.md` for any stale "not yet implemented" caveat — reconcile.
- [ ] 5.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 5.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 6. OpenSpec close-out

- [ ] 6.1 Run `openspec validate fase-3-structural-equality` before archiving.
- [ ] 6.2 Archive the change once implementation, tests, and documentation sync are complete.
