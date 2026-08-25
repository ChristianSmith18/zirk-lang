## 1. Checker

- [x] 1.1 Add `fn root_binding_mutability(&self, expr: &Expr) -> Option<Mutability>` to `crates/zirk-sema/src/checker.rs`: `Expr::Path(ident)` resolves the binding via `self.scopes.resolve` and returns `Some(resolved.binding.mutability)`; `Expr::Field(inner)` recurses into `&inner.object`; every other variant returns `None` (design D1 — deliberately conservative). Implemented as `Option<(String, Mutability)>` instead of `Option<Mutability>` — the extra `String` is the root binding's name, needed by 1.2's diagnostic ("naming the root binding"); a bare `Option<Mutability>` would have required a second, duplicate walk just to recover the name for the message.
- [x] 1.2 In `check_writable_field`, after resolving the field, call `root_binding_mutability(&field.object)`; when it returns `Some(Mutability::Strict)`, emit `codes::STRICT_ALIAS_VIOLATION` naming the root binding and pointing at `field.name.span`, with a message distinct from `check_strict_alias`'s two existing ones ("writing through a projection of an `inmut::strict` reference" rather than "producing/acquiring a mutable alias").
- [x] 1.3 Confirm `check_multi_assign` (from `fase-4d-declaraciones-multiples`) rejects this case with no code change of its own — it already calls `check_assign_target` → `check_writable_field` per position (design D4/D2). Add a test exercising it through `Stmt::MultiAssign` specifically, not just `Stmt::Assign`, so the fan-out path is verified, not assumed.
- [x] 1.4 Verify the in-constructor exemption (`in_own_constructor`, `this.field = ...` inside the field's own class's `construct`) still works unchanged: `Expr::This` returns `None` from `root_binding_mutability` (design D1), so the new check never fires there regardless of the constructor exemption — add a test confirming a normal (non-strict) constructor-field-write still compiles, guarding against a regression.

## 2. Tests

- [x] 2.1 Checker unit test: `p.x = 5;` with `p` declared `inmut::strict` is rejected with `STRICT_ALIAS_VIOLATION`.
- [x] 2.2 Checker unit test: `p.a.b.c = 5;` (multi-level projection) with `p` declared `inmut::strict` is rejected — confirms the walk isn't limited to one level.
- [x] 2.3 Checker unit test: the same write through a `mut` or plain `inmut` (non-strict) binding still compiles — confirms no over-rejection.
- [ ] 2.4 BLOCKED — see final report: `inmut::strict` (and `mut`/`inmut`) have no parameter-position syntax in the current grammar (`crates/zirk-ast/src/lib.rs`'s `Param` has no mutability field; `crates/zirk-parser/src/parser.rs`'s `parse_params` never reads `Keyword::Mut`/`Keyword::Inmut`; every parameter is bound `Mutability::Immutable` unconditionally in `crates/zirk-sema/src/checker.rs`, `check_function`/`check_lambda`). A function parameter declared `inmut::strict` cannot be written in `.zrk` source today, so this specific test cannot be authored as specified. Not implemented; needs a design decision (out of this change's scope per its own explicitly-out-of-scope list, which does not mention parameter mutability syntax at all) before it can be attempted.
- [x] 2.5 Checker unit test: `left, right = obj.x, 5;` (or equivalent) inside `Stmt::MultiAssign` where one destination is a field projection off a strict binding is rejected at that position, per task 1.3.
- [x] 2.6 End-to-end `.zrk` fixture in `crates/zirk-cli/tests/corpus/invalid/` exercising the rejected case through the full pipeline (parser → checker diagnostic), following the existing corpus fixture conventions from `fase-4d-declaraciones-multiples`.
- [x] 2.7 Run `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` and fix regressions; run `cargo clippy --workspace --all-targets` and `cargo fmt --check`.

## 3. Documentation and status sync

- [x] 3.1 Add a short note to `docs/decisions/ADR-003-investigacion-fase-4.md` (a new short section, not editing the existing extensions) recording that the `inmut::strict` projection-write gap its Phase 4d/4e exploration surfaced is now closed, and restating precisely what is still open (field-declared strictness across a projection boundary, method calls through a strict receiver) so it is not read as "reachable-alias analysis is done."
- [x] 3.2 Check `docs/handbook` for any `inmut::strict` material and confirm (or fix, if found otherwise) that none of it currently claims this projection-write case is implemented, and none needs a "not yet implemented" caveat removed (the proposal's Impact section already checked this before writing — recheck at implementation time in case handbook content changed since). (Updated `12-feature-status.md` rows for multi-decl/assign and memory/unsafe to reflect the new partial `inmut::strict` coverage; also fixed an unrelated stale bullet in `13-appendices/07-current-limitations.md` claiming `Fn(...)=>R` still "remains rejected in general type positions" — stale since `fase-4d-callables` archived, fixed here since it was found while doing this check.)
- [ ] 3.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using today's date if project-status evidence changed per the script's own check).
- [ ] 3.4 Report both the zirk-lang and zirk-lang-site revisions used so the synchronization is auditable.

## 4. OpenSpec close-out

- [ ] 4.1 Run `openspec validate fase-4e-inmut-strict-proyeccion` before archiving.
- [ ] 4.2 Archive the change once implementation, tests, and documentation sync are complete.
