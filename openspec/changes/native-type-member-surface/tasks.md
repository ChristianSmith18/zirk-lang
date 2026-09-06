# Tasks: native-type-member-surface

## 1. Integer statics and value methods

- [x] 1.1 Checker: `IntN.MIN`/`MAX`/`BITS` resolved in expression position for every width (signed and unsigned)
- [x] 1.2 Checker + lowering + runtime: `IntN.parse(text)` → `Result<IntN, ParseError>`; unsigned rejects `-`
- [x] 1.3 Checker + lowering + runtime: `abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `is_even`, `is_odd` (width-correct, `abs` traps on `MIN`)
- [x] 1.4 Checker + lowering + runtime: `bit_count`, `leading_zeros`, `trailing_zeros`, `rotate_left`, `rotate_right` (width-scoped)
- [x] 1.5 `checked_add/sub/mul/div/rem` → `Result<T, OverflowError>` reusing existing overflow detection
- [x] 1.6 `wrapping_add/sub/mul` and `saturating_add/sub/mul`
- [x] 1.7 Corpus fixture `int_members.zrk` covering several widths + invalid fixture for `abs` on `MIN` behavior
- [x] 1.8 Handbook: `02-signed-integers.md`/`03-unsigned-integers.md` statuses back to `implemented`

## 2. Float members

- [x] 2.1 `FloatN.MIN`/`MAX`/`LOWEST`/`EPSILON`/`POSITIVE_INFINITY`/`NEGATIVE_INFINITY`
- [x] 2.2 `FloatN.parse(text)` → `Result<FloatN, ParseError>`
- [x] 2.3 `abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `is_finite`, `is_infinite`, `is_negative`
- [x] 2.4 `floor`, `ceil`, `round`, `truncate`, `fraction`, `pow`, `sqrt` (negative `sqrt` is a controlled error)
- [x] 2.5 Corpus fixture `float_members.zrk`
- [x] 2.6 Handbook: `04-decimals.md` statuses

## 3. Char members

- [x] 3.1 `byte_length`, `codepoint_count`, `ascii_code`, `is_ascii`
- [x] 3.2 `is_alphabetic`, `is_numeric`, `is_alphanumeric` (+ keep `is_letter`/`is_digit` aliases), `normalize(form)` with `UnicodeNormalization`
- [x] 3.3 Corpus fixture updates in `char_methods.zrk`
- [x] 3.4 Handbook: `08-char.md` statuses

## 4. String members

- [x] 4.1 `length` (graphemes), `byte_length`, `is_empty()`
- [x] 4.2 `find(needle): Int64?`, `replace(needle, replacement)`
- [x] 4.3 `trim_start`, `trim_end`, `to_lowercase`, `to_uppercase`
- [x] 4.4 `normalize(form)`, `clone`, `split_whitespace`, `lines`
- [x] 4.5 `bytes()`/`codepoints()`/`chars()` views (`List`-backed iteration is acceptable)
- [x] 4.6 Negative indexing for `String` (`s[-1]`, slices) resolved as `length + index`
- [x] 4.7 Corpus fixture updates in `string_methods.zrk`
- [x] 4.8 Handbook: `09-string.md` statuses

## 5. Duration members

- [x] 5.1 `abs`, `sign`, `is_zero`, `is_positive`, `is_negative`
- [x] 5.2 Corpus fixture updates in `duration_literals_and_arithmetic.zrk`
- [x] 5.3 Handbook: `11-duration.md` + `03a-temporal/08-duration.md` statuses

## 6. Regex.parse

- [x] 6.1 Checker: `Regex.parse` static call → `Result<Regex, RegexError>`
- [x] 6.2 Runtime `zirk_regex_compile` returning handle-or-null + `RegexError`
- [x] 6.3 Corpus fixture `regex_parse.zrk`
- [x] 6.4 Handbook: `12-regex.md` status

## 7. Collections

- [x] 7.1 `Array(e0, …)`, `[e0, …]` literal (with `Array<T>` context), `List(e0, …)`
- [x] 7.2 Negative indexing for `Array`/`List`
- [x] 7.3 `Array` slicing `a[start:end:step]`
- [x] 7.4 `clone()` and `to_string()` for `Array`/`List`
- [x] 7.5 `List.remove(value)` overload
- [x] 7.6 Corpus fixtures `collection_literals_and_negative_index.zrk`, `collection_clone.zrk`, `collection_to_string.zrk`, `array_slicing.zrk`, `list_remove_value.zrk` and existing `array_list_basic.zrk`/`array_list_strings.zrk`
- [x] 7.7 Handbook: `01-arrays.md`/`03-lists.md` statuses

## 8. Data types

- [ ] 8.1 Tuple `length`, `to_string`
- [ ] 8.2 Traditional-enum `case.name`, `case.value`, `case.to_string()`
- [ ] 8.3 Default `to_string` for `record`/algebraic-enum/`class`/`Weak`/callable values
- [ ] 8.4 Corpus fixtures
- [ ] 8.5 Handbook statuses in `10-data-types`, `00-tuples`, `03-traditional-enums`, `04-algebraic-enums`, `01-records`, `12-function-types`, `04-safe-references`, `08-classes-and-objects`

## 9. Verification and docs

- [x] 9.1 `cargo test --workspace` green incl. new fixtures; `cargo clippy --workspace` clean
- [ ] 9.2 Handbook statuses updated for every delivered member; `13-type-member-index.md` delivery note removed where fully delivered
- [x] 9.3 `12-feature-status.md` updated; `openspec validate --all --strict` green
- [ ] 9.4 Website re-sync (`yarn content:sync` in zirk-lang-site) after committing handbook sources
- [x] 9.5 Commit(s) in zirk-lang and zirk-lang-site
- [ ] 9.6 Archive change when the user confirms
