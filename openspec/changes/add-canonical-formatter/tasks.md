## 1. Formatter engine foundations

- [x] 1.1 Add the `zirk-format` crate with documented responsibility (canonical document formatting, text-in/text-out, no IO) and workspace dependency edges from `zirk-cli`, `zirk-lsp`, and `zirk-browser`; verify `cargo build -p zirk-format` and `cargo build -p zirk-format --target wasm32-unknown-unknown` succeed.
- [x] 1.2 Add a trivia-preserving tokenization mode to `zirk-lexer` (comment tokens with spans, whitespace still implicit) without changing `tokenize`'s signature or cost; verify existing lexer tests still pass and a new test yields comments in source order.
- [x] 1.3 Implement the `Doc` pretty-printing IR (`text`/`line`/`softline`/`group`/`indent`/`join`) with the 100-column width and 4-space indent constants; verify with unit tests that a group fits flat or breaks all of its lines.
- [x] 1.4 Implement comment attachment: leading comments bind to the next construct at its indentation, trailing comments share their line with one space, list-interior comments keep their own line; verify golden tests for each attachment class including edge cases (comment after `{`, inside an empty block, file-trailing).

## 2. Canonical layout rules

- [x] 2.1 Implement declaration-level formatting: `fn` signatures, class/record/enum/trait/interface members, `construct` members, fields with defaults and modifiers; verify multi-line signatures break all parameters per the width rule.
- [x] 2.2 Implement statement and block formatting: bindings, assignments, control flow (`if`/`match`/`for`/`while`/loops), `return`/`throw`, and nested blocks; verify indentation and brace placement per the canonical rule.
- [x] 2.3 Implement expression formatting: binary/unary chains, calls, member access, lambdas, `'''` literals (keeping the existing layout rule), interpolation, and collection literals; verify a too-long call breaks all arguments and a fitting one stays inline.
- [x] 2.4 Implement blank-line collapsing (max one), trailing-whitespace removal, and tab normalization; verify golden tests.
- [x] 2.5 Preserve sugared constructs verbatim in kind: promoted constructor parameters and constructor-less classes are never expanded; verify a corpus test over `oop.zrk`-style input.

## 3. Directives and safety

- [x] 3.1 Implement `// zirk-compiler: fmt off` / `// zirk-compiler: fmt on` scanning and verbatim emission of disabled regions; verify byte-identical passthrough and the warning for `on` without `off`, unclosed `off`, and no nesting.
- [x] 3.2 Keep the unparseable-input contract: diagnostics are returned and no partial output is produced; verify an invalid file is never rewritten.
- [x] 3.3 Add the idempotence + parse-stability harness: format, re-format (identical bytes), re-parse (identical program structure); run it over `crates/zirk-cli/tests/corpus/`, every handbook example, and the site examples; resolve all violations.

## 4. CLI surface

- [x] 4.1 Re-plumb `zirk format` onto `zirk-format` for single files, preserving `--check` and the optional `.zrk` extension; verify the existing format tests still pass.
- [x] 4.2 Accept multiple files and directories with recursive `.zrk` discovery; verify per-target reporting and that one invalid target does not block the rest.
- [x] 4.3 Add `--diff` printing canonical changes without writing; verify exit codes (`0` clean/changed-write, non-zero on `--check` drift and on errors).
- [x] 4.4 Update `zirk --help` text for the new surface; verify help lists the flags.

## 5. LSP and editor

- [x] 5.1 Switch `format_source` to the full engine; verify `textDocument/formatting` returns full-document canonical edits and empty results for unformattable text via an LSP test.
- [x] 5.2 Verify the VS Code extension's format-on-save / manual format path works end-to-end; copy the extension to `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/` and validate per `AGENTS.md` if any file under `editors/vscode/` changed.

## 6. Browser operation

- [x] 6.1 Add the versioned `format` request/response types reusing the existing source-byte and diagnostic limits; verify `apiVersion` stays 1 and unknown-version requests still fail explicitly.
- [x] 6.2 Implement `format` in `zirk-browser` through the shared frontend and `zirk-format`; verify contract tests for success, diagnostics, limit failures, and pre-initialization errors.
- [ ] 6.3 Update the manifest generator (`operations` list), TypeScript declarations, and package verification; verify `verify-browser-package.mjs` and the dependency audit still pass.
- [ ] 6.4 Extend the module-worker smoke test with a `format` invocation; verify the built package formats a fixture inside a worker.
- [x] 6.5 Document the package handoff for the companion `zirk-lang-site` playground change (new package revision, manifest, Format UI wiring) in the site's delivery docs.

## 7. Documentation, status, and website sync

- [x] 7.1 Update `docs/TOOLCHAIN.md` and the handbook toolchain chapter with the full `zirk format` surface (targets, `--check`, `--diff`, directives).
- [x] 7.2 Document the canonical style rules (indent, width, breaking, comment anchoring, directives) in the handbook so the formatter's output is normative, not surprising.
- [x] 7.3 Update Feature Status, Current Limitations, and roadmaps: the formatter moves from the `'''` slice to full canonical formatting, delivered in the browser package too.
- [ ] 7.4 Reformat the repository's `.zrk` corpus and handbook examples in a single dedicated commit after the engine lands.
- [ ] 7.5 Add `zirk format --check` to `scripts/check-local.sh` and CI after the corpus reformat; verify the gate passes.
- [ ] 7.6 Commit the `zirk-lang` work on `develop`, run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` against a clean `../zirk-lang-site`, review the diff, and commit the site synchronization separately recording both revisions.

## 8. Verification

- [x] 8.1 Run `cargo test -p zirk-format -p zirk-lexer -p zirk-cli -p zirk-lsp -p zirk-browser` and resolve all failures.
- [ ] 8.2 Run `./scripts/check-local.sh` and resolve all regressions.
- [ ] 8.3 Rebuild and verify the browser package from the clean committed revision; run the WASM contract runner and worker smoke test including `format`.
- [ ] 8.4 After the companion site change lands, run the site's `yarn test` and `yarn build`; resolve content-integrity or budget regressions.
