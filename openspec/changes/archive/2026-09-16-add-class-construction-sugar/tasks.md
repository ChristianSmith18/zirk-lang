## 1. Grammar and AST

- [x] 1.1 Extend the `construct` parameter grammar to accept member modifiers (`public`/`private`/`protected`, `mut`/`inmut`/`inmut::strict`, and valid combinations) and record a `promoted` marker on the parameter node; verify `zirk check` parses `construct(public name: String, private inmut id: UInt64)` without error.
- [x] 1.2 Keep unmodified `construct` parameters ordinary: verify a bare parameter declares no field and binds only in the body via a parser/AST test.
- [x] 1.3 Add parser tests for promoted optional (`name?: Type`) and default (`name: Type = expr`) parameters; verify both forms parse and keep their optional/default contract.
- [x] 1.4 Emit a targeted diagnostic when member modifiers appear on method, free-function, lambda, or `extern` parameters; verify the diagnostic names `construct` as the only legal position.
- [x] 1.5 Emit a targeted diagnostic for a promoted variadic parameter and for promotion written outside a `class` `construct` (record/enum/interface/trait members or any other declaration); verify both diagnostics.

## 2. Field promotion semantics

- [x] 2.1 Register a promoted parameter as a class field in `zirk-sema` — same name, type, and modifiers, marked promoted with its span at the parameter; verify `construct(public name: String)` produces a member visible to `this.name` in any method.
- [x] 2.2 Emit the duplicate-field error when a promoted parameter names an already-declared field; verify `name: String;` + `construct(public name: String)` fails identifying both sites.
- [x] 2.3 Emit the multi-constructor error when promotion appears in a class declaring more than one `construct`; verify the cause advises keeping one constructor or declaring attributes explicitly, and that multiple non-promoted `construct` members still compile.
- [x] 2.4 Emit the promoted-variadic error; verify `construct(public ...rest: List<Int>)` fails.
- [x] 2.5 Verify promoted parameters work in named construction (`User(name: "Ana")`), in reordered named arguments, and with optional/default parameters; verify generic classes (`Box<T>`) promote `T`-typed parameters.
- [x] 2.6 Emit the legibility warning when a promoted field is read or written through `this.` in a method other than its declaring constructor; verify it is a warning (compilation succeeds), points at the promoted parameter, and never fires for uses inside the declaring constructor.
- [x] 2.7 Verify a promoted `inmut` field is finalized by the generated assignment under the same rules a hand-written constructor follows (assignment inside the ctor allowed, later mutation rejected).

## 3. Implicit default constructor

- [x] 3.1 Synthesize an implicit `construct() {}` in `zirk-sema` for classes declaring no `construct`, marked implicit so it carries no source span; verify `class Foo {}` + `Foo()` compiles and produces default-initialized fields.
- [x] 3.2 Remove the implicit constructor when any `construct` is declared; verify `class A { construct(x: Int) {} }` + `A()` reports "no constructor takes zero arguments" and `Foo(1)` on a constructor-less class reports "no constructor takes one argument".
- [x] 3.3 In a subclass with an accessible no-argument base constructor, make the implicit constructor call `super()` automatically and emit the legibility warning suggesting an explicit constructor; verify `class B extends A {}` + `B()` runs the base constructor.
- [x] 3.4 When the base declares only constructors requiring arguments, keep the subclass declaration legal and report the unsatisfiable base construction at the `B()` instantiation site; verify the diagnostic names the base requirement.
- [x] 3.5 Verify the implicit constructor stays invisible: not present in LSP document symbols/outline, not shown in generated member listings, and never materialized into source by any tool.
- [x] 3.6 Verify `inmut` fields and ordinary fields in an implicitly-constructed class hold their declared or type defaults, matching a hand-written `construct() {}`.

## 4. Lowering, execution, and conformance

- [x] 4.1 Lower promoted-parameter assignments as the first constructor-body statements, after any explicit `super(...)` call and after field initializers; verify with an IR or execution test that ordering matches the hand-written equivalent.
- [x] 4.2 Add a corpus program covering `ShortCreation`/`LongCreation` equivalence (same observable output and field layout) and `OmitConstruct`; verify `zirk run` produces identical observable behavior for both forms.
- [x] 4.3 Add differential fixtures so the interpreter and native pipeline agree on promoted parameters, implicit constructors, and the subclass `super()` path; verify the fixture harness passes both engines.
- [x] 4.4 Verify the browser `check`/`run` path handles sugar source identically to the CLI through the shared frontend (differential harness already covers this; add the sugar fixture to the browser-compatible set if its features allow).

## 5. Diagnostics and tooling

- [x] 5.1 Allocate stable diagnostic codes for the new errors (duplicate promoted field, multi-constructor promotion, promotion outside class `construct`, promoted variadic, unsatisfiable implicit construction) and warnings (promoted field used elsewhere, implicit `super()`); verify codes appear in `--json` output.
- [x] 5.2 Verify promoted fields appear as ordinary fields in `zirk-lsp` document symbols and hover (resolved at the parameter span) while the implicit constructor appears nowhere; add LSP tests for both.
- [x] 5.3 Verify `zirk format` leaves both sugared forms untouched (the existing formatter only rewrites `'''` layout, so a corpus test asserting no rewrite suffices here).

## 6. Documentation, status, and website sync

- [x] 6.1 Update `docs/ZIRK_LANGUAGE_SPEC.md` section 7: promoted constructor parameters, the implicit default constructor, subclass `super()` behavior, and the new diagnostics.
- [x] 6.2 Update the handbook constructors and instantiation chapters (`02-handbook/08-classes-and-objects/`) with short/long form examples, the legibility warnings, and the implicit constructor rules.
- [x] 6.3 Update Feature Status and the roadmaps (compiler + handbook + `docs/init/ZIRK_ROADMAP.md`) with the delivered sugar and its phase placement.
- [x] 6.4 Update `editors/vscode` grammar/snippets only if the parameter-modifier syntax needs highlighting changes; validate the JSON and copy to the installed extension per `AGENTS.md`.
- [x] 6.5 Commit the `zirk-lang` work on `develop`, then run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` against a clean `../zirk-lang-site` and review the generated diff.
  Evidence: sugar landed as `b193b7c feat(lang): class construction sugar`; `./scripts/sync-website-content.sh --audit-date 2026-09-16` pinned the site to `4b4d917` (372 handbook documents); the site diff was provenance/checksums plus the verified browser package acquisition.
- [x] 6.6 Commit the reviewed site synchronization separately and record both repository revisions.
  Evidence: site commits `12a80ef` (WASM package + `format` contract), `9ef8c4c` (content sync), `0d4f394` (delivery doc); the delivery pair is recorded in `zirk-lang-site/docs/playground-editor-delivery.md` (`zirk-lang` `4b4d917…` / `zirk-lang-site` `9ef8c4c…`).

## 7. Verification

- [x] 7.1 Run `cargo test -p zirk-parser -p zirk-sema -p zirk-ir -p zirk-cli -p zirk-lsp` and resolve all failures.
- [x] 7.2 Run `./scripts/check-local.sh` (fmt, clippy, builds, workspace tests) and resolve all regressions.
- [x] 7.3 Run the site's `yarn test` after the content sync when the site change lands; resolve any content-integrity regressions.
  Evidence: `yarn test` passed post-sync (47 files / 187 tests) and `yarn build` prerendered 378 routes with sitemap and shell verification green; only pre-existing bundle-budget warnings remain.
