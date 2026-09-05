## Why

Zirk's scalar and control-flow surface is now complete through Phase 4, but it still cannot express everyday data structures. Programmers can write numbers, text, and objects, but they have no `Array`, `List`, `Tuple`, `Duration`, or `Regex` to build real programs. This change closes that gap, turning the language from a core into a usable, self-contained general-purpose tool.

It also removes `value class`, which is semantically redundant with `record` and currently half-finished. Keeping it would require finishing method bodies and `implements` for a feature that `record` already covers.

## What Changes

- **BREAKING**: Remove `value class` from the grammar, AST, checker, IR, tests, and docs. Migrate every `value class` example to `record`.
- Add `Tuple(A, B, ...)` as a core value product type with constant `tuple[n]` indexing and destructuring.
- Add `Array<T>` as a contiguous, fixed-capacity/variable-length native reference collection with indexed access, mutable writes, bounds checking, `length`, `is_empty`, and `for ... in` via `Iterable<T>`.
- Add `List<T>` as a resizable native reference collection with `add`, `remove`, `insert`, indexed access, and iteration.
- Add `Duration` as a compiler primitive with literal suffixes (`250ms`, `2h`, `1.5s`), arithmetic, comparison, and `to_string`. **Not** `Period`, `Date`, `DateTime`, etc.
- Add `Regex` as a native reference type with `re'pattern'` literal syntax, `match` integration, capture groups, `find`, `replace`, `split`, and `matches` iteration.
- Add full `String` surface: write `s[i] = c`, slicing `[start:end:step]`, and methods `split`, `trim`, `search`, `contains`, `starts_with`, `ends_with`, `substring`.
- Add `Char` classification: `is_uppercase`, `is_lowercase`, `to_uppercase`, `to_lowercase`, `is_digit`, etc.
- Add `Range<T>` generic with `start`, `end`, `step`, `reverse()`, and slicing.
- Lower `type alias` so aliases are usable in executable programs, not just declarations.
- Derive `Clone` for `record` and `enum` values when every field is `Clone`.
- Lower generic contract satisfaction so user-defined generic `class`/`record` can implement `Iterable<T>` and similar generic contracts.
- Update `Iterable<T>`/`Iterator<T>` contracts to cover `Array`, `List`, `String`, `Range`, and `Regex` matches.
- Update the public handbook, feature status, current limitations, and roadmap.
- Synchronize the companion `../zirk-lang-site` repository once the code, docs, and specs are committed.

## Capabilities

### New Capabilities
- `zirk-regex`: regular expression literals, the `Regex` type, capture groups, and `match`/`find`/`replace`/`split` semantics.

### Modified Capabilities
- `zirk-collections`: add requirements for `Array<T>`, `List<T>`, `Tuple(T...)`, and `Range<T>`; remove any `value class` references from collection examples.
- `zirk-temporal-types`: add `Duration` as the first temporal primitive; leave `Date`/`Time`/`DateTime`/`Period` explicitly out of scope.
- `zirk-data-types`: remove `value class`; add `Tuple` value semantics, `type alias` lowering, derived `Clone` for `record` and `enum`, and generic contract satisfaction.
- `zirk-standard-library`: add `String` mutation, slicing, and search methods; add `Char` classification and normalization methods.
- `zirk-generics`: add generic contract/implements lowering for user-defined generic `class` and `record`.
- `zirk-type-system`: add `Base` variants for `Tuple`, `Array`, `List`, `Duration`, and `Regex`; resolve generic `Iterable<T>`/`Iterator<T>` for them.
- `zirk-ir-lowering`: add lowering for tuple construction and indexing, array/list construction, indexing, and bounds checks, range operations, duration literals and arithmetic, regex literals/operations, and string methods.
- `zirk-runtime-io`: add runtime support for `Array`/`List` allocation and GC tracking, `Duration` storage and arithmetic, regex engine integration, and string operation helpers.
- `zirk-grammar` and `zirk-lexical-syntax`: add `re'...'`, `Duration` suffixes, tuple type/construction, `Array`/`List` syntax, and `Range` literals.
- `zirk-check`: add CLI corpus fixtures for the new types and operations.

## Impact

- `crates/zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime`, `zirk-cli`.
- `docs/` and `../zirk-lang-site` for handbook and status updates.
- Runtime dependencies: likely the `regex` crate for the regex engine and a custom nanosecond representation for `Duration`.

## Impact

- `crates/zirk-lexer`, `zirk-parser`, `zirk-ast`, `zirk-sema`, `zirk-ir`, `zirk-codegen-llvm`, `zirk-runtime`, `zirk-cli`.
- `docs/` and `../zirk-lang-site` for handbook and status updates.
- Runtime dependencies: likely the `regex` crate for the regex engine and `chrono` or a custom nanosecond representation for `Duration`.
