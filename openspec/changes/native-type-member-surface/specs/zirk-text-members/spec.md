# zirk-text-members

## ADDED Requirements

### Requirement: String metadata properties

A `String` SHALL expose `length` (grapheme count) and `byte_length`
(encoded UTF-8 byte count) as properties, and `is_empty()` as a method.

#### Scenario: Grapheme length

- **WHEN** `"πa".length` is read
- **THEN** the result is `2` (`Int32`), not the byte count

#### Scenario: Byte length

- **WHEN** `"πa".byte_length` is read
- **THEN** the result is the UTF-8 byte count

#### Scenario: Emptiness

- **WHEN** `"".is_empty()` and `"x".is_empty()` run
- **THEN** they produce `true` and `false`

### Requirement: String search and replacement

A `String` SHALL expose `find(needle)` returning the first grapheme index
as `Int64?` (`null` when absent), and `replace(needle, replacement)`
returning a new `String` with every occurrence replaced.

#### Scenario: Find present and absent

- **WHEN** `"hola".find("la")` and `"hola".find("z")` run
- **THEN** they produce `2` and `null`

#### Scenario: Replace all occurrences

- **WHEN** `"a-b-a".replace("a", "x")` runs
- **THEN** the result is `"x-b-x"`

### Requirement: String trimming and case

A `String` SHALL expose `trim_start()`, `trim_end()`, `to_lowercase()`,
and `to_uppercase()` with Unicode-aware semantics, alongside the existing
`trim()`.

#### Scenario: One-sided trims

- **WHEN** `"  x  ".trim_start()` and `"  x  ".trim_end()` run
- **THEN** they produce `"x  "` and `"  x"`

#### Scenario: Unicode-aware case

- **WHEN** `"ß".to_uppercase()` runs
- **THEN** the result is `"SS"`

### Requirement: String normalization and remaining views

A `String` SHALL expose `normalize(form)` taking a
`UnicodeNormalization` value, `split_whitespace()`, `lines()`, `clone()`,
and the `bytes()`/`codepoints()`/`chars()` views.

#### Scenario: Normalization composes

- **WHEN** `"e\u{301}".normalize(NFC)`-equivalent input is normalized
- **THEN** the result is `"é"`

#### Scenario: Lines splits on terminators

- **WHEN** `"a\nb".lines()` is iterated
- **THEN** it yields `"a"` then `"b"`

#### Scenario: Clone is an independent value

- **WHEN** `s.clone()` is taken and `s` is mutated
- **THEN** the clone keeps its contents

### Requirement: Char metadata and classification

A `Char` SHALL expose `byte_length` and `codepoint_count` properties and
`ascii_code()`, `is_ascii()`, `is_alphabetic()`, `is_numeric()`,
`is_alphanumeric()`, `is_letter()`, `is_digit()`, `is_whitespace()`,
`is_uppercase()`, `is_lowercase()`, `normalize(form)`,
`to_uppercase()`, `to_lowercase()`, and `to_string()`.

#### Scenario: ASCII code

- **WHEN** `'a'.ascii_code()` runs
- **THEN** the result is `97`; a non-ASCII grapheme produces `-1`

#### Scenario: Classification

- **WHEN** `'9'.is_numeric()` and `'a'.is_alphabetic()` run
- **THEN** both produce `true`

### Requirement: Dynamic regex compilation

`Regex.parse(pattern)` SHALL compile a pattern at runtime and return
`Result<Regex, RegexError>`.

#### Scenario: Valid dynamic pattern

- **WHEN** `Regex.parse("[0-9]+")` is matched as `Ok(r)` and
  `r.matches("42")` runs
- **THEN** the result is `true`

#### Scenario: Invalid dynamic pattern

- **WHEN** `Regex.parse("(")` is evaluated
- **THEN** the result is `Error` with a `RegexError`
