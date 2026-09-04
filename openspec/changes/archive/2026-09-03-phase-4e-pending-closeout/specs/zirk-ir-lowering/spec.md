## ADDED Requirements

### Requirement: String indexing lowers to a grapheme-offset instruction
A `String[index]` read expression SHALL lower to an instruction that computes the byte offset of the `index`-th grapheme and then extracts the grapheme.

#### Scenario: Valid string index
- **WHEN** the program contains `let c: Char = s[i]` with `s` of type `String`
- **THEN** the IR contains `StringGraphemeOffset` followed by `GraphemeLenAt` and `GraphemeSlice` to produce a `Char`

#### Scenario: Out-of-bounds string index
- **WHEN** the program contains `s[i]` and `i` is greater than or equal to the grapheme count
- **THEN** the IR branches to `throw_native_failure` with `IndexOutOfBoundsError`

#### Scenario: Negative string index rejected
- **WHEN** the program contains `s[i]` with `i` of a signed type and `i < 0`
- **THEN** the compiler reports a negative-index diagnostic before lowering

#### Scenario: String is not writable through index
- **WHEN** the program contains `s[i] = c`
- **THEN** the compiler reports `INDEXING_NOT_WRITABLE` or equivalent
