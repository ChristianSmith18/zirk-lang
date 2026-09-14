# triple-quoted-strings Specification

## Purpose

Define the `'''…'''` multiline string literal: literal text with preserved
newlines, tabs and spacing, active escapes and `{expr}` interpolation, and a
compile-time left-edge rule that makes the literal's column boundary part of
the language — not just a style convention.

## ADDED Requirements

### Requirement: Triple-quoted literal

The lexer SHALL recognize a string literal delimited by `'''` on both ends.
The literal SHALL produce a `String` value. Single `'`
and double `''` quote runs inside the content SHALL be literal text; the
first `'''` run closes the literal. `''''''` SHALL be the empty string.

#### Scenario: Simple literal

- **WHEN** `'''\nhello\n'''` (a `'''` literal containing the line `hello`)
  is tokenized and evaluated
- **THEN** the value is a `String` equal to `hello`

#### Scenario: Quotes inside content

- **WHEN** content contains `it's` and `''quoted''`
- **THEN** both quote runs are part of the string and the literal is not
  terminated early

#### Scenario: Empty literal

- **WHEN** `''''''` is evaluated
- **THEN** the value is the empty `String`

### Requirement: Literal content preservation

Physical newlines, tabs, and spaces inside `'''…'''` SHALL be preserved in
the value, except for the left-edge stripping defined by the edge rule. The
newline immediately after the opening `'''` SHALL NOT be part of the value,
and the line holding the closing `'''` SHALL contribute nothing — including
its preceding newline.

#### Scenario: Newlines and blank lines preserved

- **WHEN** a literal contains the lines `a`, `` (empty), `b`
- **THEN** the value is `a\n\nb` with no trailing newline

#### Scenario: Interior spacing preserved

- **WHEN** a content line is `texto   con   espacios`
- **THEN** the interior runs of spaces are preserved verbatim

### Requirement: Escapes and interpolation

`'''` literals SHALL honor the same escape set as `"…"` strings (`\n`, `\t`,
`\r`, `\0`, `\\`, `\{`, `\}`, `\'`) and SHALL support `{expr}` interpolation
with balanced-brace scanning, desugared through `to_string()` like any other
interpolation.

#### Scenario: Escapes active

- **WHEN** a content line contains `a\tb`
- **THEN** the value contains a tab between `a` and `b`

#### Scenario: Interpolation

- **WHEN** `'''…{variable}…'''` is evaluated with `variable = "x"`
- **THEN** the interpolation renders `x` in place

#### Scenario: Literal brace escape

- **WHEN** a content line contains `\{`
- **THEN** the value contains a literal `{` and no interpolation is opened

### Requirement: Left-edge rule

The opening `'''` column SHALL define the literal's left edge, measured in
characters (tabs count as one). Every non-empty content line MUST
begin at or to the right of that column; exactly edge-width leading
whitespace SHALL be stripped from each content line. Empty lines SHALL be
exempt from the indentation requirement.

#### Scenario: Canonical form

- **WHEN** the opening `'''` sits at column 4 and content lines are indented
  4 spaces
- **THEN** the value contains each line's text with its 4-space prefix
  removed

#### Scenario: Extra indentation is content

- **WHEN** a content line is indented 2 spaces beyond the edge
- **THEN** the value keeps those 2 spaces

#### Scenario: Line left of the edge

- **WHEN** a non-empty content line starts left of the opening `'''` column
- **THEN** a diagnostic is emitted pointing at that line, naming the edge
  column, and stating that content may not escape the literal's left edge

### Requirement: Closing delimiter placement

The closing `'''` MUST appear on its own line, at or to the right of the
left edge. A `'''` literal not closed before end of file SHALL produce a
diagnostic pointing at the opening delimiter.

#### Scenario: Unclosed literal

- **WHEN** a `'''` literal reaches end of file without a closing `'''`
- **THEN** a diagnostic points at the opening `'''` and indicates the
  missing closing delimiter

#### Scenario: Closing delimiter left of the edge

- **WHEN** the closing `'''` starts left of the opening edge column
- **THEN** a diagnostic is emitted pointing at the closing delimiter

### Requirement: Canonical layout contract

The canonical layout for a `'''` literal SHALL be: the opening `'''` on the
line after the `=`, `(` or `name:` that introduces it, indented one level
deeper than that introducing line; the closing `'''` on its own line aligned
to the same column as the opening `'''`; content lines indented to that same
column. This contract is what `zirk format` enforces and what the editor
guide visualizes.

#### Scenario: Canonical shape

- **WHEN** `x =\n    '''\n    text\n    '''` is formatted
- **THEN** the delimiters share one column and `text` begins at that column

#### Scenario: Non-canonical placement is accepted but reformattable

- **WHEN** a `'''` literal opens on the same line as its `=` and the source
  is otherwise valid
- **THEN** compilation succeeds, and `zirk format` rewrites it to the
  canonical layout
