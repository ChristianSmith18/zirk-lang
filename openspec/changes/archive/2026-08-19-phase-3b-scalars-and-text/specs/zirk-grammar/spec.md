## ADDED Requirements

### Requirement: Literals of integer widths and `Float`

The grammar SHALL recognize an integer literal as any of the signed or unsigned widths when the context determines it, and a fractional literal (with optional scientific notation) as `Float`, both with `_` as a visual separator.

#### Scenario: Visual separator in a wide literal
- **WHEN** `1_000_000` is written
- **THEN** it is lexed as the integer `1000000`

#### Scenario: Scientific notation
- **WHEN** `1e2` is written
- **THEN** it is lexed as a `Float` literal with value `100.0`

### Requirement: `Char` literal

The grammar SHALL recognize a `Char` literal delimited by single quotes, capable of containing an extended Unicode grapheme of more than one code point.

#### Scenario: Literal of an ASCII character
- **WHEN** `'a'` is written
- **THEN** it is lexed as a `Char` literal

#### Scenario: Unclosed delimiter
- **WHEN** a `Char` literal does not have its closing quote before the end of the line
- **THEN** a lexical diagnostic is emitted

### Requirement: Bitwise and shift operators

The grammar SHALL recognize `&`, `|`, `^`, `~`, `<<`, `>>` as binary operators (`~` unary), at the precedence levels `ZIRK_LANGUAGE_SPEC.md` fixes for them, distinct from the logical `&&`/`||`.

#### Scenario: Precedence distinct from the logical one
- **WHEN** an expression combining `&` with `&&` is written
- **THEN** it is parsed according to the precedence of each operator, not as if they were the same

### Requirement: Interpolation in `String` literals

The grammar SHALL recognize `{expr}` within a `String` literal as an interpolated expression, with `\{` as the escape for a literal brace.

#### Scenario: Simple interpolation
- **WHEN** `"Hello, {name}"` is written
- **THEN** it is parsed as literal text plus an interpolated expression `name`

#### Scenario: Escaped brace
- **WHEN** `"\{not interpolated\}"` is written
- **THEN** it is parsed as literal text with braces, with no interpolated expression
