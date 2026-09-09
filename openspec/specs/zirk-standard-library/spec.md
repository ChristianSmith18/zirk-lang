# zirk-standard-library Specification

## Purpose

Defines the standard-library module inventory and the shared contracts for API
signatures, outcomes, permissions, cancellation, hostile-input limits,
portability, and documentation synchronization.
## Requirements
### Requirement: The standard library has one synchronized capability owner
The standard-library capability SHALL inventory every public `std` module and
shared API contract defined by `docs/ZIRK_STDLIB_SPEC.md`. Its handbook owner
pages SHALL use the same signatures, result variants, permissions,
cancellation behavior, safety limits, and platform guarantees.

#### Scenario: A module API changes
- **WHEN** a normative standard-library signature or behavior changes
- **THEN** its specialized spec, OpenSpec capability, handbook owner, indexes,
  and affected examples are updated in the same change

### Requirement: Result and stream outcomes remain distinct
Operations returning `Result<T,E>` SHALL use `Ok(T)` and `Error(E)`. A
stream-specific enum MAY use `Value(T)`, `End`, and `Error(E)` only when the
operation explicitly returns that enum, such as `ReadResult<T>` or
`ReceiveResult<T>`.

#### Scenario: HTTP request succeeds
- **WHEN** an HTTP operation returning `Result<HTTPResponse,HTTPError>` succeeds
- **THEN** documentation and implementations expose `Ok(response)`, not
  `Value(response)`

### Requirement: Privileged APIs preserve denial explicitly
Filesystem, network, process, environment, system, and secret APIs SHALL retain
permission, decoding, platform, and cancellation failures in their declared
typed outcome. Nullable/default convenience operations SHALL NOT hide denial
or host failure.

#### Scenario: Environment fallback lacks authority
- **WHEN** `Env.get_or(name, fallback)` has no matching grant
- **THEN** it returns `Error(EnvironmentPermissionError)` rather than the
  fallback

### Requirement: String write by index

`String` SHALL support `s[i] = c` where `i` is a valid grapheme index and `c` is a `Char` or a single-grapheme `String`. The assignment SHALL mutate the shared String referent, invalidate the grapheme cache, and preserve the String's invariants. Complete aliases of the same String SHALL observe the replacement. A write through `mut` or `inmut` SHALL be valid referent mutation; a write reachable through `inmut::strict` SHALL be rejected.

#### Scenario: Write a character
- **WHEN** `mut s = "abc"; s[1] = 'X';` is executed
- **THEN** `s` becomes `"aXc"`

#### Scenario: Shared alias observes an indexed write
- **WHEN** `mut text = "Hello, World!"; inmut alias = text; text[0] = "😃";` is executed
- **THEN** both `text` and `alias` produce `"😃ello, World!"`

#### Scenario: Inmut binding mutates its referent
- **WHEN** `inmut text = "abc"; text[1] = 'X';` is compiled and executed
- **THEN** compilation accepts the indexed referent mutation and `text` becomes `"aXc"`

#### Scenario: Strict reference rejects an indexed write
- **WHEN** `inmut::strict text = "abc"; text[1] = 'X';` is compiled
- **THEN** compilation reports a strict reachable-mutation diagnostic

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
