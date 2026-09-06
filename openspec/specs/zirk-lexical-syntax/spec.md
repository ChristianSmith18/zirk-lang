# zirk-lexical-syntax

## Purpose

Defines the lexicon of Zirk: tokens, literals, comments, locations and lexical errors.

The keywords of the whole language are recognized, not only those of the implemented subset, so a construct from a later phase can be told apart from a syntax error.
## Requirements
### Requirement: Tokenization of the subset

The lexer SHALL convert `.zrk` source text into a sequence of tokens, each with its location in the source.

The subset's tokens are: identifiers, keywords, integer literals, string literals, Boolean literals, operators, delimiters, and end of file.

#### Scenario: Minimal program
- **WHEN** `fn main(): Void { }` is tokenized
- **THEN** the sequence produced is: keyword `fn`, identifier `main`, `(`, `)`, `:`, type identifier `Void`, `{`, `}`, end of file

#### Scenario: Location of each token
- **WHEN** any input is tokenized
- **THEN** each token exposes its starting file, line, and column, 1-based
- **AND** the column counts Unicode characters, not bytes

### Requirement: Integer literals

The lexer SHALL recognize integer literals in decimal, hexadecimal (`0x`), and
binary (`0b`) base, allowing `_` as a separator between digits, per
`ZIRK_LANGUAGE_SPEC.md` section 3 and `docs/handbook/11-reference/04-literals.md`.

#### Scenario: Simple integer
- **WHEN** `42` is tokenized
- **THEN** an integer literal with value 42 is produced

#### Scenario: Thousands separator
- **WHEN** `1_000_000` is tokenized
- **THEN** an integer literal with value 1000000 is produced

#### Scenario: Separator in an invalid position
- **WHEN** `_1000` or `1000_` is tokenized
- **THEN** an error diagnostic with a stable code, cause, and help is emitted

#### Scenario: Hexadecimal and binary base
- **WHEN** `0xff` or `0b1010` is tokenized
- **THEN** an integer literal with value 255 and 10 respectively is produced
- **AND** no invalid-suffix diagnostic is emitted

### Requirement: String literals

The lexer SHALL recognize string literals delimited by double quotes, with escape sequences.

#### Scenario: Simple string
- **WHEN** `"Hello"` is tokenized
- **THEN** a string literal with content `Hola` is produced

#### Scenario: Escape sequences
- **WHEN** a string contains `\n`, `\t`, `\"`, or `\\`
- **THEN** the literal represents them as a newline, tab, quote, and backslash

#### Scenario: Unclosed string
- **WHEN** a string is not closed before the end of the line or the file
- **THEN** a diagnostic pointing to the string's opening is emitted
- **AND** the help indicates that the closing quote is missing

#### Scenario: Unknown escape
- **WHEN** a string contains an unrecognized escape sequence
- **THEN** a diagnostic pointing to the sequence is emitted

### Requirement: Boolean literals

The lexer SHALL recognize `true` and `false` as Boolean literals, not as identifiers.

#### Scenario: Boolean values
- **WHEN** `true` or `false` is tokenized
- **THEN** a Boolean literal is produced

### Requirement: Comments

The lexer SHALL recognize line comments `//` and block comments `/* ... */`, discarding them from the token sequence.

#### Scenario: Line comment
- **WHEN** a line contains `// text`
- **THEN** the content from `//` to the end of the line produces no tokens

#### Scenario: Block comment
- **WHEN** the source contains `/* text */`
- **THEN** the delimited content produces no tokens

#### Scenario: Unclosed block comment
- **WHEN** a block comment is not closed before the end of the file
- **THEN** a diagnostic pointing to its opening is emitted

### Requirement: Reserved words of the whole language

The lexer SHALL recognize as keywords those of the whole language, not only those of the implemented subset.

Recognizing them allows the parser to distinguish an unimplemented construct from a syntax error, and makes the diagnostic understandable.

#### Scenario: Keyword outside the subset
- **WHEN** `class`, `for`, `match`, `task`, or another keyword of the whole language is tokenized
- **THEN** the corresponding keyword token is produced
- **AND** no identifier is produced

### Requirement: Case sensitivity

The lexer SHALL distinguish uppercase from lowercase, per `ZIRK_LANGUAGE_SPEC.md` section 1.

#### Scenario: Identifier that differs only in capitalization
- **WHEN** `total` and `Total` are tokenized
- **THEN** two distinct identifiers are produced

### Requirement: Unrecognized character

The lexer SHALL emit a diagnostic for any character that does not belong to the lexicon, instead of silently discarding it.

#### Scenario: Invalid character
- **WHEN** the source contains a character that does not start any valid token
- **THEN** a diagnostic with the character's exact location is emitted

### Requirement: Fractional literal suffixes match the renamed type families

A fractional literal suffixed with `f` or `fN` SHALL be classified as a binary float literal and its type SHALL be the corresponding `FloatN` width (`Float64` for `f`, `Float16` for `f16`, `Float32` for `f32`, `Float128` for `f128`). A fractional literal suffixed with `d` or `dN` SHALL be classified as an exact decimal literal and its type SHALL be `Decimal`. An unsuffixed fractional literal SHALL be classified as `Decimal`. The token `b` and `bN` suffixes from the previous naming scheme SHALL NOT resolve and SHALL be rejected with a diagnostic pointing to `f`/`fN`.

#### Scenario: `f` suffix resolves to `Float64`

- **WHEN** the literal `1.5f` is tokenized and typed
- **THEN** the resulting type is `Float64`

#### Scenario: `f32` suffix resolves to `Float32`

- **WHEN** the literal `1.5f32` is tokenized and typed
- **THEN** the resulting type is `Float32`

#### Scenario: `d` suffix resolves to `Decimal`

- **WHEN** the literal `1.5d` is tokenized and typed
- **THEN** the resulting type is `Decimal`

#### Scenario: Unsuffixed fractional literal resolves to `Decimal`

- **WHEN** the literal `1.5` is tokenized and typed without context
- **THEN** the resulting type is `Decimal`

#### Scenario: Old `b` suffix is rejected

- **WHEN** the literal `1.5b` is tokenized
- **THEN** a diagnostic states that the binary suffix is now `f` and the exact decimal suffix is `d`

### Requirement: Inclusive range token

The lexer SHALL recognize `..=` as its own token, distinct from `..` and from `.`, applying longest-match.

#### Scenario: Inclusive range
- **WHEN** `0..=10` is tokenized
- **THEN** the inclusive-range tokens are produced, not `..` followed by `=`

### Requirement: Power tokens

The lexer SHALL recognize `**` and `**=` as their own tokens, applying the
longest-match rule before multiplication. These tokens belong to the
implemented subset: the lexer SHALL NOT attribute an arrival phase to them
and the compiler SHALL NOT defer them with a phase diagnostic.

#### Scenario: Power

- **WHEN** `value ** 2` or `value **= 2` is tokenized
- **THEN** the power and compound-power tokens are produced
- **AND** two consecutive multiplication tokens are NOT produced

#### Scenario: Power tokens carry no arrival phase

- **WHEN** the phase of `**` or `**=` is queried
- **THEN** it reports that the token is part of the implemented subset, not a
  later phase

### Requirement: Bitwise and shift operators

The lexer SHALL recognize `&`, `|`, `^`, `~`, `<<`, `>>` and their compound
assignment forms, present at levels 7 through 10 and 18 of
`docs/handbook/11-reference/02-operators-and-precedence.md`.

#### Scenario: Bitwise operator
- **WHEN** `flags & mask` or `value << 2` is tokenized
- **THEN** the corresponding operator token is produced
- **AND** no unrecognized-character diagnostic is emitted

#### Scenario: Logical conjunction versus bitwise
- **WHEN** `a && b` and `a & b` are tokenized
- **THEN** two distinct tokens are produced, by longest-match

### Requirement: Character literals

The lexer SHALL recognize character literals delimited by single quotes,
preserving their Unicode content intact for the grapheme validation performed
by semantic analysis.

The lexer SHALL NOT decide whether the content is exactly one grapheme: that
check belongs to `Char`'s semantics.

#### Scenario: ASCII character
- **WHEN** `'a'` is tokenized
- **THEN** a character literal with that content is produced

#### Scenario: Composite grapheme
- **WHEN** a character literal contains a family emoji formed by several code points
- **THEN** a single character literal that retains all its code points is produced

#### Scenario: Unclosed character literal
- **WHEN** a character literal reaches the end of the line or the file without its closing quote
- **THEN** a diagnostic pointing to its opening is emitted

### Requirement: Regex literals

The lexer SHALL recognize regex literals delimited as `re'pattern'`,
preserving escapes for the regular-expression parser.

#### Scenario: Regex literal
- **WHEN** the source contains `re'^[0-9]+$'`
- **THEN** a single regex literal with the pattern text is produced

#### Scenario: Unclosed regex
- **WHEN** a regex literal reaches the end of the line or the file without its closing quote
- **THEN** a diagnostic pointing to the `re'` opening is emitted

### Requirement: Interpolation in string literals

The lexer SHALL recognize `{ expression }` interpolation within a string
literal, with balanced braces, producing the literal parts and the embedded
expressions as distinct elements of the literal.

The same balancing rules SHALL apply to an interpolated range bound such as
`0..{number}`.

#### Scenario: Interpolated string
- **WHEN** `"value={value}"` is tokenized
- **THEN** the literal keeps the textual part and the embedded expression separately
- **AND** the braces are NOT part of the text

#### Scenario: Nested braces
- **WHEN** an interpolation itself contains balanced braces
- **THEN** the interpolation closes at its matching brace, not at the first one

#### Scenario: Unclosed interpolation
- **WHEN** an interpolation does not close before the end of the literal
- **THEN** a diagnostic pointing to its opening is emitted

#### Scenario: Interpolated range bound
- **WHEN** the source contains `0..{number}.step(1)`
- **THEN** the token stream retains the interpolated bound as an expression within the range

### Requirement: Duration literals

The lexer SHALL recognize duration literals formed by a number and one of the
suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, or `w`, allowing a negative
sign, without treating `m` as a calendar month.

#### Scenario: Duration suffixes
- **WHEN** the source contains `10ns`, `500ms`, `2h`, `3d`, or `-3s`
- **THEN** each is emitted as a duration literal with its unit

#### Scenario: A calendar month is not a duration
- **WHEN** a developer needs a calendar month
- **THEN** the documented form is a `Period` constructor, not a duration suffix

### Requirement: Remaining keywords of the whole language

The lexer SHALL also recognize `do`, `yield`, `interface`, and `trait` as
keywords of the whole language.

Recognizing them is what allows `interface User { }` to produce "not
implemented yet" instead of a syntax error on an identifier.

`strict` and `value` SHALL NOT be reserved words. Both appear in the language
only in a fixed position — `inmut::strict` and `value class` — and reserving
them would invalidate `mut value = 1;` and `match r { Ok(value) => ... }`,
which are ordinary Zirk and appear in the spec's own examples. They are
recognized by position.

#### Scenario: Contract from a later phase
- **WHEN** `interface` or `trait` is tokenized
- **THEN** the corresponding keyword token is produced
- **AND** no identifier is produced

#### Scenario: Post-condition loop
- **WHEN** `do { } while pending;` is tokenized
- **THEN** `do` and `while` are produced as keywords

#### Scenario: Contextual word as an identifier
- **WHEN** `mut value = 1;` or `mut strict = true;` is tokenized
- **THEN** `value` and `strict` are produced as identifiers

### Requirement: Correct phase attribution per token

Every keyword and every operator outside the implemented subset SHALL declare
the phase the roadmap assigns to it.

#### Scenario: Error-handling keyword
- **WHEN** the phase of `default` is queried
- **THEN** it declares the phase of `try`/`catch`, not that of decorators

#### Scenario: Pipe operator
- **WHEN** the phase of `|>` is queried
- **THEN** it declares the phase of the functional style, not that of objects

