## 1. Remove `value class`

- [x] 1.1 Remove the contextual `value class` parser branch and `ClassKind::ValueClass` from `zirk-ast`.
- [x] 1.2 Remove `value class` handling from `zirk-sema` checker, lowering, and diagnostics.
- [x] 1.3 Remove `value class` handling from `zirk-ir` lowering and `zirk-codegen-llvm`.
- [x] 1.4 Delete or migrate all `value class` CLI corpus fixtures and unit tests to `record` or `class`.
- [x] 1.5 Remove `value class` from the handbook and current limitations.
- [x] 1.6 Run `cargo test --workspace` and `openspec validate --all --strict`.

## 2. Tuples

- [x] 2.1 Add `Tuple(...)` to the grammar as a type expression and value construction `(a, b, ...)`.
- [x] 2.2 Add `Base::Tuple` to `zirk-sema` and resolve tuple types with constant `tuple[n]` indexing.
- [x] 2.3 Add tuple destructuring in variable declarations and `match` patterns.
- [x] 2.4 Lower tuple construction to an LLVM struct value and `tuple[n]` to constant `extractvalue`.
- [x] 2.5 Add CLI corpus fixtures for tuple construction, indexing, and destructuring.

## 3. `Array<T>`

- [x] 3.1 Add `Array<T>` to the grammar and `Base::Array` to `zirk-sema` with generic element type.
- [x] 3.2 Implement `Array<T>(capacity)` construction.
- [x] 3.3 Add runtime `zirk_rt_array_*` functions for allocation, indexing, bounds checks, and GC tracing.
- [x] 3.4 Lower `Array` indexing, `length`, `is_empty`, and bounds-checked read/write.
- [x] 3.5 Wire `Array<T>` to `Iterable<T>` through a dedicated lowering pass (`lower_for_in_array_list`).
- [x] 3.6 Add CLI corpus fixtures for `Array` construction, indexing, mutation, and iteration.

## 4. `List<T>`

- [x] 4.1 Add `List<T>` to the grammar and `Base::List` to `zirk-sema`.
- [x] 4.2 Implement `List<T>()` construction and methods `add`, `insert`, `remove`, `length`, `is_empty`.
- [x] 4.3 Add runtime `zirk_rt_list_*` functions for resizable backing store and GC tracing.
- [x] 4.4 Lower `List` method calls, indexed access, and iteration.
- [x] 4.5 Wire `List<T>` to `Iterable<T>` through the same collection loop lowering.
- [x] 4.6 Add CLI corpus fixtures for `List` construction, resize, mutation, and iteration.

## 5. `Duration`

- [x] 5.1 Add `Duration` suffix tokens (`ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w`) to the lexer.
- [x] 5.2 Add `Duration` to the parser and `Base::Duration` to `zirk-sema` with `i64` nanosecond representation.
- [x] 5.3 Implement `Duration` arithmetic, comparison, negation, scalar multiply/divide, and `Duration / Duration`.
- [x] 5.4 Add runtime `zirk_rt_duration_*` helpers for arithmetic and overflow checks.
- [x] 5.5 Implement `Duration.to_string()` with normalized unit output.
- [x] 5.6 Add CLI corpus fixtures for `Duration` literals, arithmetic, and printing.

## 6. `Regex`

- [x] 6.1 Add the `re'pattern'` literal to the lexer and parser.
- [x] 6.2 Add `Regex` to `zirk-sema` as a native reference type.
- [x] 6.3 Add the `regex` crate dependency and runtime `zirk_rt_regex_*` helpers.
- [x] 6.4 Implement `Regex.matches`, `Regex.find`, `Regex.replace`, and `Regex.split` (`matches`, `replace` and `find` done; `find` returns `Regex.Match?`; `split` pending `List<T>`).
- [x] 6.5 Implement `Regex.Match` with `group(n)`, `group(name)`, `start`, `end`, and `text`.
- [x] 6.6 Wire regex match iteration to `for ... in`. Implemented as `Regex.find_all(text): List<Regex.Match>` (a `List` is already `Iterable`-compatible in `for ... in` lowering), because `Regex.matches(text)` already returns `Boolean` per the "Match testing" requirement; `specs/zirk-regex/spec.md` updated to name `find_all` as the iteration entry point.
- [x] 6.7 Add `match` expression integration for `re'pattern'` patterns with bindings (`Pattern::Regex` in `zirk-ast`, `re'pat' name` binds the `Regex.Match`; checker validates the pattern at compile time and requires a `String` scrutinee; lowering reuses `zirk_regex_find`).
- [x] 6.8 Add CLI corpus fixtures for regex literals, find, replace, split, iteration, and capture groups (`regex_basics` covers literals, matches, replace and find; `regex_groups` covers capture groups; `regex_split` covers split; `regex_find_all` covers match iteration; `regex_match_arm` covers `re'pattern'` match arms).

## 7. Iterable / Iterator update

- [x] 7.1 Ensure `Array<T>`, `List<T>`, `String`, and `Range` satisfy `Iterable<T>` (`for x in collection` is lowered for each).
- [x] 7.2 Verify `for x in collection` lowers for `Array<T>` and `List<T>`.
- [x] 7.3 Add unit tests for `Iterator<T>` `next()` and `has_next()` on collections.

## 8. Documentation and status updates

- [x] 8.1 Update `docs/handbook` to document `Tuple`, `Array`, `List`, `Duration`, and `Regex`.
- [x] 8.2 Remove `value class` from `docs/handbook` and `docs/init/ZIRK_AGENT_PROMPT.md`.
- [x] 8.3 Update `docs/init/ZIRK_FEATURE_STATUS.md`, `docs/handbook/11-reference/12-feature-status.md`, and `docs/handbook/13-appendices/07-current-limitations.md`.
- [x] 8.4 Update `docs/init/ZIRK_ROADMAP.md` if Phase 7 wording changes.

## 9. Validation and website sync

- [x] 9.1 Run `cargo clippy --workspace` and fix warnings.
- [x] 9.2 Run `cargo test --workspace` with `LLVM_SYS_201_PREFIX` exported.
- [x] 9.3 Run `openspec validate --all --strict` and fix failures.
- [ ] 9.4 Run `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD` and review `../zirk-lang-site` diff.
- [ ] 9.5 Commit `zirk-lang`, then commit `zirk-lang-site` if accepted.
- [ ] 9.6 Archive the OpenSpec change.

## 10. String and Char operations

- [x] 10.1 Implement `String` write `s[i] = c` with grapheme cache invalidation.
- [x] 10.2 Implement `String` slicing `[start:end:step]` returning a new `String`.
- [x] 10.3 Add `String` methods: `trim()`, `search(pattern)`, `contains(pattern)`, `starts_with(pattern)`, `ends_with(pattern)`, `substring(start, end)`, `split(separator): List<String>`.
- [x] 10.4 Add `Char` classification: `is_uppercase`, `is_lowercase`, `is_digit`, `is_whitespace`, `is_letter`.
- [x] 10.5 Add `Char` normalization: `to_uppercase`, `to_lowercase`.
- [x] 10.6 Add CLI corpus fixtures for `String` and `Char` operations.

## 11. `Range<T>`

- [x] 11.1 Add `Range<T>` to the grammar and `Base` with `start`, `end`, and `step`.
- [x] 11.2 Implement `Range` construction `start..end` and `start..end..step`.
- [x] 11.3 Implement `Range.reverse()` and slicing on ranges.
- [x] 11.4 Wire `Range<T>` to `Iterable<T>` for numeric and `Duration` `T`.
- [x] 11.5 Add CLI corpus fixtures for `Range`.

## 12. `type alias` lowering and generic contracts

- [x] 12.1 Lower `type alias` so it does not fail with `NOT_LOWERED` in executable programs.
- [x] 12.2 Derive `Clone` for `record` and `enum` when every field is `Clone`.
- [x] 12.3 Implement generic contract satisfaction lowering for user-defined `class`/`record` that implement generic contracts like `Iterable<T>`.
- [x] 12.4 Add CLI corpus fixtures for `type alias`, `Clone` derivation on record/enum, and generic contracts.

## 13. Final validation and docs

- [x] 13.1 Update `07-current-limitations.md` to remove the debts closed by this change.
- [x] 13.2 Update `docs/handbook` for `String` methods, `Range`, `type alias`, `Clone`, and generic contracts.
- [x] 13.3 Run the full test suite and fix regressions.
- [ ] 13.4 Sync `../zirk-lang-site` if any public status changed.
