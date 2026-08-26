## 1. Checker (zirk-sema)

- [ ] 1.1 Remove the `not_lowered` gate for `ClassKind::Abstract` in `check_class` (`crates/zirk-sema/src/checker.rs`, ~line 2474-2483).
- [ ] 1.2 Trace and verify the general class-declaration path's method-index/`overridden`-flag assignment (whatever code builds a flattened method list for `ClassKind::Class` and assigns `index`/`overridden`) actually runs for `ClassKind::Abstract` too, and produces a usable shared virtual-index scheme across every concrete adopter — do not assume this "just works" because `CallVirtual` itself is declaration-kind-agnostic (design D1's own explicit warning: the native hierarchy's index assignment was hand-written and may have taken shortcuts the general path doesn't already replicate).
- [ ] 1.3 Verify method-index consistency when one concrete class adopts a user abstract class AND a plain interface/trait in the same `implements` list (design D3/Risk) — both must dispatch correctly without index collisions.
- [ ] 1.4 Checker unit tests: a user abstract class with an `abstract fn` requirement, adopted by two different concrete classes, each with its own override — confirm both are accepted and produce consistent method indices; a concrete class implementing both an abstract class and an interface together.

## 2. IR (zirk-ir) — verification, not new mechanism (design expects `CallVirtual` needs no change)

- [ ] 2.1 Verify a call through a value statically typed as a user abstract class lowers to `InstKind::CallVirtual` (`crates/zirk-ir/src/lower.rs`, ~line 5370-5389) exactly the way a call through `Throwable` already does — confirm by an explicit test, not by reading the code and assuming it applies.
- [ ] 2.2 IR-level test: two concrete adopters of the same abstract class, each with a distinct override — confirm the lowered `CallVirtual` correctly resolves to each adopter's own override at its own call site (i.e., the dispatch is receiver-type-driven, not resolved once at compile time to a single target).

## 3. Cross-cutting correctness tests (do not skip)

- [ ] 3.1 End-to-end `.zrk` fixture: declare `abstract class Shape { abstract fn area(): Float64; }`, two concrete adopters (`Circle`, `Square`) each with their own `override fn area()`, a function taking `Shape` and calling `.area()` on it, called once with each concrete type — confirm each call dispatches to the correct adopter's own implementation. This is the single most important test in this change, mirroring the existing working `Throwable` case but for user code.
- [ ] 3.2 End-to-end `.zrk` fixture: a concrete class implementing both a user abstract class and a plain interface, calling a method from each through their respective abstract/interface-typed variables.
- [ ] 3.3 Confirm no regression in the native exception hierarchy's own dynamic dispatch (`catch Throwable(e) { e.message() }`-style tests) — run the existing corpus fixtures that exercise this, not just the new ones.
- [ ] 3.4 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 4. Documentation and status sync

- [ ] 4.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — drop "abstract-class dynamic dispatch" from the "Several Phase 3 constructs..." bullet/row once delivered.
- [ ] 4.2 Check `docs/handbook` for any existing abstract-class teaching material with a stale "not yet implemented" caveat — reconcile.
- [ ] 4.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 4.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 5. OpenSpec close-out

- [ ] 5.1 Run `openspec validate fase-3-abstract-dispatch` before archiving.
- [ ] 5.2 Archive the change once implementation, tests, and documentation sync are complete.
