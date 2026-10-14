# Tasks — inferred-return-types

## 1. Parser — optional annotation

- [x] 1.1 Make `FnDecl.return_type` and `MethodDecl.return_type` `Option<TypeRef>` in `crates/zirk-ast`; split `parse_return_type` into `parse_optional_return_type` (returns `None` when no `:` follows) and a `require_return_type` wrapper emitting `MISSING_RETURN_TYPE`; verify `cargo test -p zirk-ast -p zirk-parser` compiles with the field type change.
- [x] 1.2 Use `parse_optional_return_type` in `parse_fn` and `parse_method` (non-abstract); keep `require_return_type` in `parse_contract_method`, `parse_extern_fn`, `parse_fn_dec`, and `parse_method` when `is_abstract`; verify parser tests accept `fn f() { }` and still reject `extern "C" fn f(x: Int32);` without a type.
- [x] 1.3 Repurpose `MISSING_RETURN_TYPE`'s message/help for the bodyless case ("a signature without a body must declare its return type"); verify the code's fixtures still produce the diagnostic for interface members, abstract methods, and extern declarations.

## 2. Sema — inference pass

- [x] 2.1 Register unannotated signatures with `returns: Type::UNKNOWN` + a `pending_inference` marker in `declare_function` and `declare_members`; verify a `fn f() { }` program declares a pending signature without diagnostics.
- [x] 2.2 Implement `infer_return_types`: per pending function/method, check its body in collect mode (`check_return` records `(span, Type)` instead of enforcing; `current_return` = infer sentinel, `expected_type` = `None` on returned exprs), `unify` the collected types, write the resolved type into the `Signature`; `Void` when no valued return and the body may complete, `Never` when it always diverges; verify `fn f() { return 5 }` resolves `Int32` and `fn f() { }` resolves `Void` in unit tests.
- [x] 2.3 Emit `INCOMPATIBLE_INFERRED_RETURNS` when `unify` fails, naming both types; verify a `Int32`/`String` mix produces the diagnostic and checking continues with `UNKNOWN`.
- [x] 2.4 Demand-driven recursion: call resolution on a `pending_inference` signature triggers its inference; an `inference_in_progress` set detects cycles and emits the cannot-infer diagnostic naming the cycle + "annotate one member" help; verify `fn a() { return b(); } fn b() { return a(); }` fails and annotating either member fixes it; `fn f(n: Int32) { return f(n-1); }` fails with the recursion diagnostic.
- [x] 2.5 Save/restore per-function inference state (`current_return`, `in_main_scope`, `pending_throws`, `unsafe_depth`) around nested `infer_one` calls, mirroring the existing `pending_throws` save/restore; verify a nested inference does not leak state into the outer body check.
- [x] 2.6 Post-inference enforcement: validate each recorded `(span, actual)` against the resolved type and apply the all-paths rule — identical diagnostics to a declared annotation; verify `fn f() { return 5; return; }` reports the bare `return;` as `Void` where `Int32` is expected, and `fn f(c: Boolean) { if c { return 5; } }` reports `MISSING_RETURN`.
- [x] 2.7 Mark bodies already checked during inference so `check_function`/`check_class` do not re-check them; verify no diagnostic is emitted twice for a function inferred on demand.
- [x] 2.8 Reorder the checker pipeline: `infer_return_types` after all declaration passes, `check_conformance`/`require_pending_overrides`/`infer_method_mutation`/`check_entrypoint` after inference; verify `cargo test -p zirk-sema` stays green and a `main` inferred non-`Void` still fails `INVALID_ENTRYPOINT`.
- [x] 2.9 Method inference: `#override`/contract-implementing unannotated methods infer first, then conformance compares the resolved signature; verify an inferred-`Int32` override against an `Int64` requirement reports the existing signature-mismatch diagnostic, and a matching inferred type passes.

## 3. Type map + `zirk check --types`

- [x] 3.1 Add a `type_map: Vec<TypeMapEntry>` to the checker recording `(FileId, Span)` → `{kind, rendered_type, inferred}` for declaration names (`fn`/method/`mut`/`inmut`/param/field) and resolved expressions; verify entries exist for both declared and inferred items in unit tests.
- [x] 3.2 Emit the map through `zirk check --types` as structured JSON sibling to diagnostics, frontend-only; verify `zirk check --types main.zrk` prints entries like `{"kind":"function","name":"hola","type":"fn(): Void","inferred":true}` and `cargo build -p zirk-cli --bin zirk-check --no-default-features` still works without LLVM.

## 4. VS Code extension — hover

- [x] 4.1 Register a `HoverProvider` for `.zrk` in `editors/vscode/extension.js`: fetch `zirk check --types` inside the existing post-save bridge refresh, cache per-document, resolve position → smallest containing span → hover markdown (Zirk code block + `(inferred)` marker); verify hovering `hola` in `fn hola() { }` shows `fn hola(): Void` marked inferred, and `count` in `mut count: Int32 = 0;` shows `count: Int32`.
- [x] 4.2 Graceful degradation: keep the previous cache on type-map failure, no hover on no match; verify a malformed `--types` response never produces an incorrect hover.
- [x] 4.3 Bump the extension to `0.5.0` (`package.json`, README notes); copy `editors/vscode/*` to `~/.devin/extensions/christiansmith.zirk-lang-0.5.0/` per AGENTS.md and validate the JSON/JS.

## 5. Fixtures + docs + closeout

- [x] 5.1 Corpus fixtures: `valid/` for `fn main()`, inferred `Int32`, method inference, trait-default inference, forward-call inference; `invalid/` for inference cycles, incompatible returns, inferred-override mismatch, bodyless signatures without annotation; verify `cargo test -p zirk-cli` green.
- [x] 5.2 Update `docs/init/ZIRK_FEATURE_STATUS.md` (inference + hover rows, bump "Last updated"), `README.md` if it names the mandatory annotation, and handbook function-declaration pages for the optional annotation; verify `zirk check` passes on every touched `.zrk` example.
- [x] 5.3 `cargo test --workspace` green; `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] 5.4 Commit; `./scripts/sync-website-content.sh` (with `--audit-date` if status text changed); commit `../zirk-lang-site` separately and record both revisions; `openspec validate inferred-return-types --strict`.
