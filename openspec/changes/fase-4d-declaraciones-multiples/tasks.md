## 1. AST

- [x] 1.1 Add `Stmt::MultiLet(MultiLetStmt)` to `crates/zirk-ast/src/lib.rs`: `{ mutability: Mutability, names: Vec<Ident>, ty: Option<TypeRef>, inits: Vec<Expr>, span: Span }` (empty `inits` = no initializer list; distinct from a 1-arity `inits` for `LetStmt` compatibility — do not reuse `LetStmt`).
- [x] 1.2 Add `Stmt::MultiAssign(MultiAssignStmt)`: `{ targets: Vec<AssignTarget>, values: Vec<Expr>, span: Span }`, preserving both lengths even when they differ (arity check happens in the checker, per design D1).
- [x] 1.3 Wire both into `Stmt::span()` and any exhaustive `match Stmt` in `zirk-ast` itself (e.g. visitor/walker helpers, if present).

## 2. Parser

- [x] 2.1 In the `mut`/`inmut`/`inmut::strict` declaration parse path, after the first identifier, look ahead for a comma; if present, collect a comma-separated list of simple identifiers before the required `:` type annotation. Reject a non-identifier (e.g. `mut a, this.b: T;`) with a clear diagnostic rather than falling through to a confusing parse error.
- [x] 2.2 Parse the optional initializer: if `=` follows the type annotation, parse a comma-separated expression list (any arity — do not require it to match the name-list arity here; that's the checker's job per design D1/D5). Build `Stmt::MultiLet` when the name list has length > 1; otherwise keep building the existing `Stmt::Let` (design D2 — no behavior change for the single-name case).
- [x] 2.3 In the assignment statement path (near `crates/zirk-parser/src/parser.rs:2105` and `2122`), after parsing the first assignable expression, look ahead for a comma before `=`; if present, collect a comma-separated list of assignable places (reuse `as_assignable`, `crates/zirk-parser/src/parser.rs:2937`), each rejected individually with the existing "not an assignable expression" diagnostic if it fails `as_assignable`.
- [x] 2.4 After `=`, parse a comma-separated expression list on the right for the multi-target case. Build `Stmt::MultiAssign` when the target list has length > 1; otherwise keep building `Stmt::Assign` (design D2).
- [x] 2.5 Confirm the parenthesized-tuple surface (if any exists elsewhere in the grammar) is untouched and cannot be confused with the bare comma-list form (design Risk #2) — add a parser test asserting `(a, b) = ...` and `a, b = ...` parse to different AST shapes if tuple destructuring syntax exists today; otherwise note in the test file that no such surface exists yet. (No tuple surface exists in `zirk-ast`/`zirk-parser` today — noted in a comment in `grammar.rs` next to the simultaneous-assignment tests.)
- [x] 2.6 Parser unit tests in `crates/zirk-parser`: shared-type declaration with no initializer, with matching initializer list, with mismatched-arity initializer list (parses, arity preserved, no parser-level error); simultaneous assignment (swap case), and mismatched-arity assignment (parses, both arities preserved per the grammar spec's "Assignment arity mismatch" scenario).

## 3. Checker (zirk-sema)

- [x] 3.1 Add checker handling for `Stmt::MultiLet`: resolve the shared type annotation (or infer from the initializer list's first entry if annotation is absent, matching whatever inference rule single `Let` already uses), then declare each name in `names` with that type and the declaration's `mutability`.
- [x] 3.2 If `inits` is non-empty, require `inits.len() == names.len()`; on mismatch emit the new dedicated diagnostic (task 5.1) instead of a generic type/arity error. On match, type-check each `inits[i]` against the shared type positionally (same conversion/coercion rules single-target `Let` already applies).
- [x] 3.3 If `inits` is empty, initialize each binding with the declared type's default independently (reuse whatever default-value mechanism class-attribute defaults already use, per `zirk-type-system`'s "Default initialization" requirement).
- [x] 3.4 Add checker handling for `Stmt::MultiAssign`: require `targets.len() == values.len()`; on mismatch emit the new dedicated diagnostic (task 5.2) instead of a generic error.
- [x] 3.5 On arity match, resolve each target to a place and reject duplicate destinations (compare resolved place identity pairwise — same binding, or same field/index projection off an aliasing base) with a clear diagnostic naming both positions.
- [x] 3.6 For each `(target, value)` pair, run exactly the single-assignment checker rules already applied to `Stmt::Assign` (`inmut`/`inmut::strict` rebinding rejection, `inmut::strict` projection-mutation rejection, positional type-check) — factor the existing single-target rule body into a function callable once per position if it is not already isolated (design D4).
- [x] 3.7 Checker unit tests in `crates/zirk-sema/tests`: multi-let type/permission fan-out, initializer-arity-mismatch diagnostic, missing-initializer defaults, multi-assign arity-mismatch diagnostic, duplicate-destination rejection, `inmut`/`inmut::strict` rebinding rejection in a multi-assign position, `inmut::strict` projection-write rejection in a multi-assign position. (The last sub-case is not covered: verified against baseline `main` that `Stmt::Assign` itself does not yet reject a field write through an `inmut::strict` base reference — e.g. `p.x = 5;` with `p` declared `inmut::strict` compiles today. `Checker::check_writable_field` only checks the field's own `inmut`, never the base reference's strict-aliasing. Design D4 says multi-assign "reuses exactly the single-assignment rule set", so `check_assign_target` reuses `check_writable_field` unchanged — same behavior, same gap, faithfully reproduced rather than newly invented here. Flagged as a pre-existing gap outside this change's scope, not a regression.)

## 4. IR / Codegen

- [x] 4.1 In `crates/zirk-ir`'s statement lowering, add `Stmt::MultiLet` lowering: for each name, allocate storage as single-name `Let` lowering already does, then store the (possibly defaulted) checked value.
- [x] 4.2 Add `Stmt::MultiAssign` lowering (design D3): evaluate `values[0..n]` into IR temporaries in left-to-right order *before* emitting any store; then emit the store for `targets[0..n]` in left-to-right order using the corresponding temporary, reusing the existing single-target store-emission path per destination (place resolution already checked in 3.6).
- [x] 4.3 Verify (add a codegen/execution test) that `left, right = right, left;` swaps correctly end-to-end — this is the concrete case that would fail under naive left-to-right evaluate-and-store-immediately lowering. (Covered by `crates/zirk-cli/tests/corpus/valid/multi_decl_assign.zrk` / `.out`, run through `zirk-cli`'s end-to-end corpus test — verified manually with the built `zirk` binary: prints `4` then `3` after `left, right = right, left;` starting from `left=3, right=4`.)
- [x] 4.4 Confirm no `zirk-codegen-llvm`-specific change is needed beyond what IR lowering already guarantees (temporaries are ordinary IR locals); if the backend has any statement-level special-casing for `Assign`, extend it or confirm `MultiAssign` lowers through the same generic path. (Confirmed: `zirk-codegen-llvm` has no `Stmt`/AST-level matching at all — it only sees `InstKind`/`Terminator`, so `MultiAssign` and `MultiLet` lower through the same generic `Store`/`StoreField`/`Load` path as everything else.)

## 5. Diagnostics

- [x] 5.1 Add a new diagnostic code (next free `E0...` in `crates/zirk-sema`, following existing numbering) for declaration-initializer arity mismatch: names the declared binding count and the initializer count, points at both spans.
- [x] 5.2 Add a new diagnostic code for simultaneous-assignment arity mismatch: names the destination count and the source count, points at both spans, matching the grammar spec's "Assignment arity mismatch" scenario intent.
- [x] 5.3 Add diagnostic golden/snapshot tests for both, consistent with how existing `E0` diagnostics are tested in this repo. (This repo has no `insta`/snapshot crate; its "golden" mechanism is `crates/zirk-cli/tests/corpus/invalid/*.zrk` + `end_to_end.rs`'s full-diagnostic-render assertions — added `multi_let_arity_mismatch.zrk` and `multi_assign_arity_mismatch.zrk` there, plus `multi_assign_duplicate_target.zrk` for the cross-destination check.)

## 6. Cross-cutting tests

- [x] 6.1 Add an end-to-end `.zrk` fixture (wherever the repo keeps compiler integration fixtures) exercising: shared-type declaration, declaration with matching initializer list, simultaneous swap, and a rejected case (duplicate destination or `inmut::strict` mutation) to confirm the diagnostic surfaces through the full pipeline. (`crates/zirk-cli/tests/corpus/valid/multi_decl_assign.zrk` for the accepted cases; `crates/zirk-cli/tests/corpus/invalid/{multi_let_arity_mismatch,multi_assign_arity_mismatch,multi_assign_duplicate_target}.zrk` for the rejected ones.)
- [x] 6.2 Run the full workspace test suite (`LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`) and fix regressions. (Full `cargo test --workspace` run: every crate reports `0 failed`; `zirk-cli`'s 26-test end-to-end suite — which runs the whole valid/invalid corpus, including the new fixtures — passes.)

## 7. Documentation and status sync

- [ ] 7.1 Update `docs/handbook/11-reference/12-feature-status.md` row for "multiple declarations and simultaneous assignment" from "defined / not implemented" to delivered, matching the actual scope shipped.
- [ ] 7.2 Update `docs/init/ZIRK_ROADMAP.md` Phase 4d status line to record both slices (`fase-4d-callables` and this change) complete, removing "remain pending, tracked as a separate change."
- [ ] 7.3 Add or update a handbook example demonstrating the syntax if the handbook documents this construct elsewhere with a "not yet implemented" caveat.
- [ ] 7.4 Commit the zirk-lang documentation/status changes, then run `./scripts/sync-website-content.sh` from the repo root; if project-status evidence changed, review `../zirk-lang-site`'s site-owned status catalog and pass `--audit-date YYYY-MM-DD` with today's date.
- [ ] 7.5 Report both the zirk-lang and zirk-lang-site revisions used so the synchronization is auditable.

## 8. OpenSpec close-out

- [ ] 8.1 Run `openspec validate --change fase-4d-declaraciones-multiples` (or repo-equivalent) before archiving.
- [ ] 8.2 Archive the change once implementation, tests, and documentation sync are complete.
