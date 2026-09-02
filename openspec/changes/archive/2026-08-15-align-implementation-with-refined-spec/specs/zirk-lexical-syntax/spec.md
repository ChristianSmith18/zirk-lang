## MODIFIED Requirements

### Requirement: Integer literals

The lexer SHALL recognize integer literals in decimal, hexadecimal (`0x`),
and binary (`0b`) base, admitting `_` as a separator between digits, per
`ZIRK_LANGUAGE_SPEC.md` section 3 and
`docs/handbook/11-reference/04-literals.md`.

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
- **AND** NO invalid-suffix diagnostic is emitted

## ADDED Requirements

### Requirement: Fractional literals and scientific notation

The lexer SHALL recognize fractional and scientific-notation literals as
`Float` literals, distinct from an integer followed by a member access.

Without this rule, `1.5` is tokenized as `1`, `.`, and `5`, which is the
worst way to fail: the language cannot say "not yet" about something it
does not even see.

#### Scenario: Fractional literal
- **WHEN** `1.5` is tokenized
- **THEN** a single Float literal is produced
- **AND** the sequence integer, dot, integer is NOT produced

#### Scenario: Scientific notation
- **WHEN** `6.02e23` or `1e2` is tokenized
- **THEN** a single Float literal with its exponent is produced

#### Scenario: Width suffix
- **WHEN** `1.5f32` is tokenized
- **THEN** the literal retains the requested width for semantic checking

#### Scenario: Float deferred to its phase
- **WHEN** a Float literal appears in a program from a phase that does not implement the `Float` family
- **THEN** the diagnostic names the literal and states the phase in which it arrives

### Requirement: Power tokens

The lexer SHALL recognize `**` and `**=` as tokens in their own right,
applying the longest-match rule ahead of multiplication.

#### Scenario: Power
- **WHEN** `value ** 2` or `value **= 2` is tokenized
- **THEN** the power and compound-power tokens are produced
- **AND** two consecutive multiplication tokens are NOT produced

### Requirement: Bitwise and shift operators

The lexer SHALL recognize `&`, `|`, `^`, `~`, `<<`, `>>` and their compound
assignment forms, present at levels 7 through 10 and 18 of
`docs/handbook/11-reference/02-operators-and-precedence.md`.

#### Scenario: Bitwise operator
- **WHEN** `flags & mask` or `value << 2` is tokenized
- **THEN** the corresponding operator token is produced
- **AND** an "unrecognized character" diagnostic is NOT emitted

#### Scenario: Logical AND versus bitwise AND
- **WHEN** `a && b` and `a & b` are tokenized
- **THEN** two distinct tokens are produced, by longest match

### Requirement: Character literals

The lexer SHALL recognize character literals delimited by single quotes,
preserving their Unicode content whole for the grapheme validation done by
semantic analysis.

The lexer SHALL NOT decide whether the content is exactly one grapheme:
that check belongs to `Char`'s semantics.

#### Scenario: ASCII character
- **WHEN** `'a'` is tokenized
- **THEN** a character literal with that content is produced

#### Scenario: Compound grapheme
- **WHEN** a character literal contains a family emoji formed of several code points
- **THEN** a single character literal that retains all its code points is produced

#### Scenario: Unterminated character literal
- **WHEN** a character literal reaches end of line or end of file without its closing quote
- **THEN** a diagnostic pointing at its opening is emitted

### Requirement: Regex literals

The lexer SHALL recognize regex literals delimited as `re'pattern'`,
preserving the escapes for the regular expression parser.

#### Scenario: Regex literal
- **WHEN** the source contains `re'^[0-9]+$'`
- **THEN** a single regex literal with the pattern text is produced

#### Scenario: Unterminated regex
- **WHEN** a regex literal reaches end of line or end of file without its closing quote
- **THEN** a diagnostic pointing at the `re'` opening is emitted

### Requirement: Interpolation in string literals

The lexer SHALL recognize `{ expression }` interpolation inside a string
literal, with balanced braces, producing the literal parts and the embedded
expressions as elements of the literal distinct from one another.

The same balancing rules SHALL apply to an interpolated range bound such as
`0..{number}`.

#### Scenario: Interpolated string
- **WHEN** `"value={value}"` is tokenized
- **THEN** the literal retains the textual part and the embedded expression separately
- **AND** the braces are NOT part of the text

#### Scenario: Nested braces
- **WHEN** an interpolation itself contains balanced braces
- **THEN** the interpolation closes at its matching brace and not at the first one

#### Scenario: Unterminated interpolation
- **WHEN** an interpolation does not close before the end of the literal
- **THEN** a diagnostic pointing at its opening is emitted

#### Scenario: Interpolated range bound
- **WHEN** the source contains `0..{number}.step(1)`
- **THEN** the token stream keeps the interpolated bound as an expression within the range

### Requirement: Duration literals

The lexer SHALL recognize duration literals formed by a number and one of
the suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, or `w`, admitting a
negative sign, without treating `m` as a calendar month.

#### Scenario: Duration suffixes
- **WHEN** the source contains `10ns`, `500ms`, `2h`, `3d`, or `-3s`
- **THEN** each is emitted as a duration literal with its unit

#### Scenario: A calendar month is not a duration
- **WHEN** a developer needs a calendar month
- **THEN** the documented form is a `Period` constructor, not a duration suffix

### Requirement: Remaining keywords of the full language

The lexer SHALL also recognize `do`, `yield`, `interface`, and `trait` as
keywords of the full language.

Recognizing them is what allows `interface Usuario { }` to produce "not yet
implemented" instead of a syntax error about an identifier.

`strict` and `value` SHALL NOT be reserved words. Both appear in the
language only in a fixed position -- `inmut::strict` and `value class` --
and reserving them would invalidate `mut value = 1;` and
`match r { Ok(value) => ... }`, which are ordinary Zirk and appear in the
spec's own examples. They are recognized by position.

#### Scenario: Contract from a later phase
- **WHEN** `interface` or `trait` is tokenized
- **THEN** the corresponding keyword token is produced
- **AND** an identifier is NOT produced

#### Scenario: Post-condition loop
- **WHEN** `do { } while pending;` is tokenized
- **THEN** `do` and `while` are produced as keywords

#### Scenario: Contextual word as identifier
- **WHEN** `mut value = 1;` or `mut strict = true;` is tokenized
- **THEN** `value` and `strict` are produced as identifiers

### Requirement: Correct phase attribution per token

Every keyword and every operator outside the implemented subset SHALL
declare the phase the roadmap assigns to it.

#### Scenario: Error-handling keyword
- **WHEN** `default`'s phase is queried
- **THEN** it declares the `try`/`catch` phase, not that of decorators

#### Scenario: Pipe operator
- **WHEN** `|>`'s phase is queried
- **THEN** it declares the functional-style phase, not that of objects

### Requirement: Inclusive range token

The lexer SHALL recognize `..=` as a token in its own right, distinct from
`..` and from `.`, applying longest match.

#### Scenario: Inclusive range
- **WHEN** `0..=10` is tokenized
- **THEN** the inclusive-range tokens are produced, not `..` followed by `=`

## REMOVED Requirements

### Requirement: Confirmed authorial tokens and literals

**Reason**: It declared intent -- `**`, `**=`, `do`/`gen`/`yield`, `..=`, and
regex literals -- without the scenarios that make it verifiable. This
change's detailed requirements cover it entirely and with testable cases,
and keeping the rule in two places would only let them drift apart.

**Migration**: `**` and `**=` move to *Power tokens*; regex literals and
their unterminated-literal diagnostic, to *Regex literals*; `do`, `gen`, and
`yield`, to *Remaining keywords of the full language*; `..=`, to *Inclusive
range token*.

### Requirement: Range interpolation tokens

**Reason**: Brace balancing for an interpolated range bound is the same
mechanism as for an interpolated string, and describing it separately
invited the two to diverge.

**Migration**: Covered by the *Interpolated range bound* scenario of
*Interpolation in string literals*.

### Requirement: Float and temporal literal vocabulary

**Reason**: It mixed two rules from different layers: duration suffixes,
which are lexical, and recognition of the `Float*` type names, which
belongs to the type system.

**Migration**: Duration suffixes move to *Duration literals*, including that
`m` means minutes and never months. Recognition of the `Float*` names lives
in *`Float` family and temporal types recognized as pending*, in
`zirk-type-system`.

### Requirement: Grapheme character literal

**Reason**: It duplicated, on the lexical side, the grapheme rule that
`zirk-type-system` already defines for `Char`.

**Migration**: Whole preservation of Unicode content moves to *Character
literals*, with the family emoji as its scenario. What counts as a grapheme
remains in *Grapheme Char*, in `zirk-type-system`.
