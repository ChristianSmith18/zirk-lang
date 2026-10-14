# Tasks

## 1. Lexer — `'''` literal

- [x] 1.1 Add `'''…'''` scanning to `crates/zirk-lexer/src/lib.rs`, producing the existing `Str`/`InterpolatedStr` token shapes with `StrPart` parts (no new expression form downstream).
- [x] 1.2 Reuse `escape()` and `interpolation()` for `'''` content; keep `'`, `''` runs literal; first `'''` closes; `''''''` is the empty literal.
- [x] 1.3 Implement the left-edge rule: record the opening `'''` column (in characters, tab = 1), strip that exact prefix from each non-empty content line, exempt empty lines.
- [x] 1.4 Emit diagnostics: unclosed `'''`, content line starting left of the edge (new code), closing `'''` left of the edge.
- [x] 1.5 Unit tests in `zirk-lexer`: tokenization, escapes, interpolation parts, edge stripping, all diagnostics, `''''''`, `'`/`''` inside content.

## 2. Checker — universal `to_string` surface

- [x] 2.1 Widen `is_printable`/`require_printable` in `crates/zirk-sema/src/checker.rs` so every type except `Void`/`Never` is printable; keep a dedicated diagnostic for those two.
- [x] 2.2 Accept `x.to_string()` on `T?` receivers (return `String`); keep `x?.to_string()` returning `String?`.
- [x] 2.3 Add `to_string` entries to the member tables of `Map`, `Set`, `Range`, `Regex`, `Channel`, `Job`, `Pointer`, `NativeSlice`, `NativeSliceMut`, `Dependent`, `Pin`.
- [x] 2.4 Verify `Base::Param` with a contract constraint declaring `to_string` dispatches through the contract.
- [x] 2.5 Unit tests in `zirk-sema`: `to_string()` accepted on every type row of the render table; `Void`/`Never` rejected; nullable accepted directly and via `?.`.

## 3. Lowering — new render arms

- [x] 3.1 `lower_to_string` in `crates/zirk-ir/src/lower.rs`: add arms for `Nullable` (`<Null>` or inner render), `Map`, `Set`, `Range`, `Regex`, `Job`, `Channel`, `Pointer`, `NativeSlice`, `NativeSliceMut`, `Dependent`, `Pin`.
- [x] 3.2 Range arm: concatenate `start`, `..`/`..=`, `end`, and `:step` when present.
- [x] 3.3 Map/Set arms: iterate entries, render `k: v`/`v`, sort by the natural order of the key/element type (rendered-text fallback), join with `, `, wrap in `{` `}`; `{}` when empty.
- [x] 3.4 Regex arm: call `zirk_regex_to_string` and wrap in `re'` + `'` via `Concat`.
- [x] 3.5 Callable arm: `<Function name>` for named function values (resolve the symbol name), `<Function>` for anonymous ones; replace the `<Callable>` constant.
- [x] 3.6 Enum arm: extend `lower_enum_to_string` so payload variants render `Variant(field: v, …)` through `lower_default_render_to_string`-style field emission.
- [x] 3.7 Surface names: strip the `$local$N` suffix in `lower_default_render_to_string`; keep `Outer.Inner`; exclude synthesized `outer` fields of `inner class`es.
- [x] 3.8 Lowering tests in `zirk-ir`: every new arm emits the expected instruction sequence/shape.

## 4. Runtime — iteration support

- [x] 4.1 Expose `Map`/`Set` entry iteration entry points in `crates/zirk-runtime` if none exist (`zirk_map_*`/`zirk_set_*` cursor or snapshot functions).
- [x] 4.2 Runtime tests: iteration snapshots for Map/Set; `zirk_regex_to_string` output.

## 5. End-to-end corpus — the render table

- [x] 5.1 Add `crates/zirk-cli/tests/corpus/valid/` cases covering every row: scalars (all int widths, radix, `Float16/32/64/128`, `Decimal`), `Boolean`, `Char`, `String`, `Duration`, `Date`, `Time`, `DateTime`, `Array`, `List` (incl. nested, empty, `String` elements), `Tuple`, `record`, `class` with and without `to_string`, local class (no `$local$`), nested/inner classes (no `outer` field), traditional and payload enums, `Result` `Ok`/`Error`, contract dispatch, named function `<Function name>`, lambda `<Function>`, `Weak`, `Map`, `Set`, `Range` (all four spellings), `Regex`, `null`/`<Null>`, `T?` present and absent, `Job`, `Channel`.
- [x] 5.2 Add corpus cases proving `println(x)` and `"{x}"` work for previously unprintable types (List, Map, Set, Range, nullable).
- [x] 5.3 Add `crates/zirk-cli/tests/corpus/invalid/` cases: `Void`/`Never` printing, `'''` edge violations, unclosed `'''`.
- [x] 5.4 Regenerate affected `.out` files (enum payload change, e.g. `tuple_enum_and_rendering.out`); run `cargo test -p zirk-cli` and the other crates per AGENTS.md.

## 6. `zirk format` — canonical formatter, first slice

- [x] 6.1 Add `format` subcommand to `zirk-cli` (in-place rewrite; `.zrk` optional like other subcommands).
- [x] 6.2 Implement the `'''` layout rule: opening `'''` moves to the line after the `=`/`(`/`name:` one indent deeper; closing `'''` aligned to the same column; content re-indented relative to the edge.
- [x] 6.3 Implement `--check` (report-only, non-zero exit when changes are needed).
- [x] 6.4 Refuse to modify files with syntax errors; report diagnostics instead.
- [x] 6.5 Tests: canonical layout after format, idempotence (second run = no diff), `--check` exit codes, error files untouched, non-`'''` style untouched.

## 7. VS Code extension

- [x] 7.1 `editors/vscode/syntaxes/zirk.tmLanguage.json`: add `string.quoted.triple.zirk` with `begin: '''` / `end: '''` and the existing interpolation patterns nested inside.
- [x] 7.2 `editors/vscode/extension.js`: implement the edge-guide decoration — scan for `'''` pairs, paint a `border-left` at the delimiter column on every spanned line, repaint on edits.
- [x] 7.3 Evaluate adding `["'''", "'''"]` to `language-configuration.json` `brackets`; ship only if it does not interfere with `'` char literals/auto-closing.
- [x] 7.4 Validate JSON/JS, then copy the extension to `~/.devin/extensions/christiansmith.zirk-lang-0.4.2/` per AGENTS.md and verify the guide + highlighting on a live file.

## 8. Documentation

- [x] 8.1 New handbook page explaining `'''` literals: literal content, left-edge rule, canonical layout, interpolation, escapes, diagnostics.
- [x] 8.2 Update `docs/handbook/02-handbook/03-everyday-types/09-string.md` and the member-index/conversions pages with the universal `to_string` contract and render table.
- [x] 8.3 Update `docs/ZIRK_LANGUAGE_SPEC.md` (lexical + `to_string` contract), `docs/ZIRK_STDLIB_SPEC.md` section 3, `docs/ZIRK_COMPILER_SPEC.md` section 10 (first `zirk format` slice delivered).
- [x] 8.4 Update `docs/handbook/11-reference/12-feature-status.md` and `13-appendices/07-current-limitations.md` (universal printing, `'''`, `zirk format` scope).
- [x] 8.5 After committing docs on a clean tree, run `./scripts/sync-website-content.sh` and pass `--audit-date` if project-status evidence changed.

## 9. Final verification

- [x] 9.1 `cargo test -p zirk-lexer -p zirk-parser -p zirk-sema -p zirk-ir -p zirk-cli` all green.
- [x] 9.2 `cargo clippy` clean.
- [x] 9.3 Manual end-to-end pass over the full render table in a scratch `.zrk` file, comparing actual output to the spec table row by row.
