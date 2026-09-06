# zirk-standard-library Specification

## Purpose

Defines the standard library surface for `String` and `Char` that is exposed as native operations.

## ADDED Requirements

### Requirement: String write by index

`String` SHALL support `s[i] = c` where `i` is a valid grapheme index and `c` is a `Char` or a single-grapheme `String`. The assignment SHALL invalidate the grapheme cache and SHALL preserve the `String`'s invariants.

#### Scenario: Write a character
- **WHEN** `mut s = "abc"; s[1] = 'X';` is executed
- **THEN** `s` becomes `"aXc"`

#### Scenario: Write out of bounds
- **WHEN** `s[10] = 'X';` is executed on a shorter `String`
- **THEN** an `IndexOutOfBoundsError` is produced

### Requirement: String slicing

`String` SHALL support `s[start:end:step]` where `start`, `end`, and `step` are integer or defaulted values. The result SHALL be a new `String` containing the selected graphemes.

#### Scenario: Basic slice
- **WHEN** `"hello"[1:4]` is evaluated
- **THEN** the result is `"ell"`

#### Scenario: Slice with step
- **WHEN** `"abcdef"[::2]` is evaluated
- **THEN** the result is `"ace"`

### Requirement: String search methods

`String` SHALL provide `contains(pattern): Boolean`, `starts_with(pattern): Boolean`, `ends_with(pattern): Boolean`, and `search(pattern): Int32?` returning the first grapheme index of a match or `null`.

#### Scenario: Contains
- **WHEN** `"hello".contains("ell")` is evaluated
- **THEN** the result is `true`

#### Scenario: Search
- **WHEN** `"hello".search("ll")` is evaluated
- **THEN** the result is `2`

### Requirement: String split and trim

`String` SHALL provide `split(separator): List<String>` and `trim(): String`. `split` with an empty separator SHALL split into graphemes. `trim` SHALL remove leading and trailing whitespace.

#### Scenario: Split
- **WHEN** `"a,b,c".split(",")` is evaluated
- **THEN** the result is `List<String>` with `"a"`, `"b"`, `"c"`

#### Scenario: Trim
- **WHEN** `"  hello  ".trim()` is evaluated
- **THEN** the result is `"hello"`

### Requirement: String substring

`String` SHALL provide `substring(start, end): String` returning the graphemes from `start` (inclusive) to `end` (exclusive), with the same bounds semantics as slicing.

#### Scenario: Substring
- **WHEN** `"hello".substring(1, 4)` is evaluated
- **THEN** the result is `"ell"`

### Requirement: Char classification

`Char` SHALL provide `is_uppercase(): Boolean`, `is_lowercase(): Boolean`, `is_digit(): Boolean`, `is_letter(): Boolean`, `is_whitespace(): Boolean`, and `is_alphanumeric(): Boolean`.

#### Scenario: Digit check
- **WHEN** `'5'.is_digit()` is evaluated
- **THEN** the result is `true`

#### Scenario: Letter check
- **WHEN** `'@'.is_letter()` is evaluated
- **THEN** the result is `false`

### Requirement: Char normalization

`Char` SHALL provide `to_uppercase(): String` and `to_lowercase(): String` returning the Unicode upper/lower equivalent. Multi-grapheme results from normalization are allowed and return `String`.

#### Scenario: Uppercase
- **WHEN** `'a'.to_uppercase()` is evaluated
- **THEN** the result is `"A"`
