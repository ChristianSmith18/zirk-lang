## MODIFIED Requirements

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
