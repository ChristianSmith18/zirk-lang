## MODIFIED Requirements

### Requirement: String indexing lowers to a grapheme-offset instruction
A `String[index]` read expression SHALL lower to an instruction that computes the byte offset of the `index`-th grapheme and then extracts the grapheme. An indexed String assignment SHALL preserve that bounds-check path and lower to a runtime effect on the receiver's stable String handle, not to an unsupported write or a replacement stored only in one local slot. When the assignment occurs inside an active unsafe transaction, lowering SHALL journal the receiver's backing-reference update before the mutation.

#### Scenario: Valid string index
- **WHEN** the program contains `let c: Char = s[i]` with `s` of type `String`
- **THEN** the IR contains `StringGraphemeOffset` followed by `GraphemeLenAt` and `GraphemeSlice` to produce a `Char`

#### Scenario: Out-of-bounds string index
- **WHEN** the program contains `s[i]` and `i` is greater than or equal to the grapheme count
- **THEN** the IR branches to `throw_native_failure` with `IndexOutOfBoundsError`

#### Scenario: Negative string index rejected
- **WHEN** the program contains `s[i]` with `i` of a signed type and `i < 0`
- **THEN** the compiler reports a negative-index diagnostic before lowering

#### Scenario: Indexed assignment mutates the String referent
- **WHEN** the program contains a checker-approved `s[i] = c`
- **THEN** the IR calls the String mutation runtime entry point without storing a replacement handle back into only `s`'s local slot

#### Scenario: Unsafe indexed assignment is journaled
- **WHEN** a checker-approved `s[i] = c` occurs in an active uncommitted `unsafe` block
- **THEN** the IR records the String backing-reference state before invoking the mutation entry point
