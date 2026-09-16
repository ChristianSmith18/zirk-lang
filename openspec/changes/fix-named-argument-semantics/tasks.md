## 1. Parse and check named arguments

- [x] 1.1 Parse `name:` by synthesizing the equivalent identifier path while preserving spans, and verify parser tests cover shorthand in free, method, and constructor calls with `cargo test -p zirk-parser`.
- [x] 1.2 Track the first named argument in shared argument matching and reject every following positional argument with a targeted diagnostic; verify focused semantic tests cover explicit and shorthand named forms using `cargo test -p zirk-sema`.
- [x] 1.3 Preserve existing duplicate-label, unknown-label, spread, optional, default, and variadic diagnostics while adding the ordering rule; verify their focused semantic regression tests pass with `cargo test -p zirk-sema`.

## 2. Resolve constructors once

- [x] 2.1 Reject duplicate positional constructor signatures, then evaluate arity-compatible candidates by labels, types, optional parameters, defaults, and variadics without leaking diagnostics from rejected candidates; verify declaration and overload tests with `cargo test -p zirk-sema`.
- [x] 2.2 Diagnose no matching constructor and genuinely ambiguous constructor calls deterministically; verify both diagnostics with focused `zirk-sema` tests.
- [x] 2.3 Extend checked-program construction metadata to retain the selected constructor index together with its class identity, and verify nested and generic construction tests continue to pass with `cargo test -p zirk-sema`.

## 3. Lower checked bindings uniformly

- [x] 3.1 Refactor held argument lowering to take a resolved parameter signature and default source rather than a free-function lookup, preserving source evaluation order and cross-branch holding; verify `cargo test -p zirk-ir`.
- [x] 3.2 Route method calls through the shared named-argument lowering path, including direct and virtual dispatch; verify reordered labels reach the intended method parameters with `cargo test -p zirk-ir`.
- [x] 3.3 Route class construction through the shared path and consume the checker-selected constructor index instead of resolving by arity; verify reordered constructor arguments and overload selection with `cargo test -p zirk-ir`.
- [x] 3.4 Route trait `super.method(...)`, class `super.method(...)`, and base-constructor `super(...)` calls through the shared path; verify reordered labels and block-opening argument expressions with `cargo test -p zirk-ir`.

## 4. End-to-end regressions

- [x] 4.1 Add parser and checker regressions for shorthand, missing shorthand bindings, repeated labels, and positional-after-named rejection; verify `cargo test -p zirk-parser -p zirk-sema`.
- [x] 4.2 Add deterministic CLI corpus programs covering reordered free-function, method, constructor, `super` method, and base-constructor calls; verify `cargo test -p zirk-cli` and run each accepted fixture with `./target/debug/zirk run` after building the CLI.
- [x] 4.3 Add an executable constructor-overload fixture that proves labels and types select the intended body, plus an ambiguity fixture that fails during `zirk check`; verify the observed output and diagnostic.
- [ ] 4.4 Run the affected compiler suite and quality gates with `cargo test -p zirk-parser -p zirk-sema -p zirk-ir -p zirk-cli`, `cargo fmt --all --check`, and `cargo clippy --workspace --all-targets -- -D warnings`.
  Evidence: the affected suite passed, but the repository-wide format gate reports unrelated formatting in `zirk-cli`, `zirk-diagnostics`, and `zirk-lsp`; workspace Clippy is blocked by the pre-existing duplicate `tests` module in `zirk-runtime/src/regex.rs`. The scoped affected-crate Clippy gate passed.

## 5. Documentation and release synchronization

- [x] 5.1 Review and update `docs/ZIRK_LANGUAGE_SPEC.md` and `docs/handbook/02-handbook/07-functions/05-named-arguments.md` so their examples and delivery wording match the verified implementation; validate documentation links and examples.
- [x] 5.2 Update the OpenSpec task evidence and validate the change with `openspec validate fix-named-argument-semantics --strict`.
- [x] 5.3 Commit the verified Zirk implementation and public documentation on the intended branch, then record the resulting revision before website synchronization.
  Evidence: committed on `develop` as `70184ec fix(sema): bind named arguments uniformly`.
- [x] 5.4 With a clean `../zirk-lang-site` checkout, run `./scripts/sync-website-content.sh`; review the site diff and validate its generated content.
  Evidence: synchronization completed from root revision `70184ec97396f4047b1d8976a04e54e92eb17ac3`; the site test suite passed.
- [x] 5.5 If project-status evidence changed, have a human review the site-owned status catalog and rerun synchronization with `--audit-date YYYY-MM-DD`; otherwise document why no audit date was required.
  Evidence: the project-status source checksums and audit date were unchanged; synchronization only updated the source revision provenance, so no new audit date was required.
- [x] 5.6 Commit the reviewed `zirk-lang-site` synchronization separately, record both repository revisions, and report that no remote publication occurred unless separately authorized.
  Evidence: site revision `c990e29 docs: sync named argument semantics`; no remote publication was performed.
