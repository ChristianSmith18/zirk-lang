# Proposal: native-type-member-surface

## Why

The handbook documents a per-type member surface (integer/Float math and
bit APIs, `Char` classification, `String` inspection/transformation,
`Duration` sign helpers, `Regex.parse`, tuple/collection literals,
`clone`/`to_string`) whose `Status` column said "implemented" while the
compiler rejected nearly all of it. The audit that corrected the doc left
the real gap: those members must actually exist. This change delivers the
documented member surface for the base/native types so the language matches
its own reference.

## What Changes

- Integer family (`Int8`…`Int128`, `UInt8`…`UInt128`): static members
  `MIN`/`MAX`/`BITS`/`parse`, and value methods `abs`, `sign`, `min`, `max`,
  `clamp`, `is_zero`, `is_even`, `is_odd`, `bit_count`, `leading_zeros`,
  `trailing_zeros`, `rotate_left`, `rotate_right`, `checked_*` (add, sub,
  mul, div, rem), `wrapping_*` (add, sub, mul), `saturating_*` (add, sub,
  mul), and `to_string()`.
- Float family (`Float16`…`Float128`): static members `MIN`/`MAX`/`LOWEST`/
  `EPSILON`/`POSITIVE_INFINITY`/`NEGATIVE_INFINITY`/`parse`, and value
  methods `abs`, `sign`, `min`, `max`, `clamp`, `is_zero`, `floor`, `ceil`,
  `round`, `truncate`, `fraction`, `is_finite`, `is_infinite`,
  `is_negative`, `pow`, `sqrt`, `to_string`.
- `Char`: `byte_length`, `codepoint_count`, `ascii_code`, `is_ascii`,
  `is_alphabetic`, `is_numeric`, `is_alphanumeric`, `normalize(form)`;
  existing `is_letter`/`is_digit` gain the documented aliases.
- `String`: `length`, `byte_length`, `is_empty`, `find`, `replace`,
  `trim_start`, `trim_end`, `to_lowercase`, `to_uppercase`,
  `normalize(form)`, `clone`, `split_whitespace`, `lines`, and the
  `bytes()`/`codepoints()`/`chars()` views.
- `Duration`: `abs`, `sign`, `is_zero`, `is_positive`, `is_negative`.
- `Regex`: `Regex.parse(pattern): Result<Regex, RegexError>`.
- `Tuple`: `length`, `to_string`.
- `Array<T>`/`List<T>`: element-listing construction (`Array(e0, …)` /
  `List(e0, …)`), `clone`, `to_string`, negative-from-end indexing, array
  slicing, `List.remove(value)`.
- Traditional enums: `case.name`, `case.value`, `case.to_string()`.
- `record`/algebraic-enum/class/`Object`/`Weak`/`callable` `to_string`
  where the doc claims a universal member.

Explicitly out of scope: the universal `.type` member (needs a runtime
`Type` object — its own change), union value delivery (unions are not
compilable yet), `String` grapheme `chars()`/`bytes()`/`codepoints()`
returning lazy `Iterator`s beyond what `Iterable` already supplies, and the
eager functional collection combinators (`map`/`filter`/`reduce`/…).

## Capabilities

### New Capabilities

- `zirk-scalar-members`: static and value member surface of the integer
  and float families (constants, inspection, checked/wrapping/saturating
  arithmetic, parsing).
- `zirk-text-members`: `Char` and `String` member surface (metadata
  properties, classification, normalization, search/replace/trim/case,
  views, `clone`), plus `Regex.parse` dynamic pattern compilation.

### Modified Capabilities

- `zirk-temporal-types`: `Duration` sign and magnitude member surface.
- `zirk-collections`: `Array`/`List` literal construction, `clone`,
  `to_string`, negative indexing, array slicing, `List.remove(value)`.
- `zirk-data-types`: tuple `length`/`to_string`, traditional-enum
  `name`/`value`/`to_string`, universal `to_string` on structured types.

## Impact

- `zirk-sema`: member recognition for scalar/char/string/duration/tuple/
  enum receivers and for `TypeName.member`/`TypeName(...)` static calls.
- `zirk-ir`: lowering dispatch for the new members; new extern decls.
- `zirk-runtime`: new `zirk_int_*`, `zirk_float_*`, `zirk_char_*`,
  `zirk_str_*`, `zirk_duration_*`, `zirk_regex_parse`, and collection
  clone/render helpers.
- `zirk-codegen-llvm`: no new instruction kinds expected; all members lower
  to existing call/arith shapes.
- `zirk-cli`: corpus fixtures per family.
- Handbook: `Status` columns flipped back to `implemented` where delivered.
- Website: handbook re-sync after the doc updates.
