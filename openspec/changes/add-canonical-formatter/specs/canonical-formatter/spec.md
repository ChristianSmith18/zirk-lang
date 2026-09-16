## MODIFIED Requirements

### Requirement: `zirk format` subcommand

The CLI SHALL expose `zirk format <target>...` accepting one or more files
and directories; directories are scanned recursively for `.zrk` files, and
a bare file argument MAY omit the `.zrk` extension. Every well-formed
target is rewritten in place to canonical form.

#### Scenario: Unparseable file

- **WHEN** `zirk format` receives syntax errors in a target
- **THEN** it reports diagnostics and leaves that file unchanged
- **AND** remaining targets are still processed

#### Scenario: Directory target

- **WHEN** `zirk format src/` runs
- **THEN** every `.zrk` file under `src/` is formatted recursively

## ADDED Requirements

### Requirement: Canonical whitespace and indentation

The formatter SHALL normalize indentation to four spaces per level, SHALL
normalize intra-line spacing around delimiters, operators, and
punctuation, and SHALL collapse runs of blank lines to at most one. Tab
characters and trailing whitespace SHALL NOT survive formatting.

#### Scenario: Inconsistent indentation normalized

- **WHEN** a block mixes two-, eight-, and tab-indented lines
- **THEN** every line is re-indented to four spaces per nesting level

#### Scenario: Operator spacing

- **WHEN** source contains `a  +b*   c`
- **THEN** the output reads `a + b * c`

#### Scenario: Blank line runs collapse

- **WHEN** three consecutive blank lines appear between members
- **THEN** the output keeps exactly one

### Requirement: Width-driven all-or-nothing breaking

When a construct exceeds the canonical line width, the formatter SHALL
break *all* of its elements onto their own lines — parameter lists,
argument lists, collection literals, and similar comma-separated groups —
rather than filling partial lines. Constructs that fit SHALL stay on one
line. The canonical line width SHALL be a documented constant.

#### Scenario: Long argument list breaks fully

- **WHEN** a call's argument list exceeds the line width
- **THEN** every argument occupies its own indented line with a trailing
  comma style defined by the canonical rule

#### Scenario: Short list stays inline

- **WHEN** an argument list fits the line width
- **THEN** it remains on a single line

### Requirement: Comments are preserved and re-anchored

The formatter SHALL preserve every comment's text and SHALL re-anchor it
by position: a comment on its own line keeps the indentation level of the
construct it precedes, and a comment sharing a line with code stays on
that line separated by a single space. Comment content SHALL NOT be
rewritten or rewrapped.

#### Scenario: Leading comment follows its construct

- **WHEN** `// explains the method` precedes a method that is re-indented
- **THEN** the comment moves to the method's new indentation level

#### Scenario: Trailing comment keeps one space

- **WHEN** `mut x = 1    // note` is formatted
- **THEN** the output reads `mut x = 1 // note`

#### Scenario: Comment inside a broken list

- **WHEN** a comment sits between two arguments of a list that breaks
- **THEN** the comment keeps its own line between them at the list's
  indentation

### Requirement: Formatting directives protect verbatim regions

The directive comments `// zirk-compiler: fmt off` and
`// zirk-compiler: fmt on` SHALL disable and re-enable formatting; source
between them SHALL be emitted byte-for-byte. An `on` without a preceding
`off` SHALL produce a warning, and an unclosed `off` SHALL extend to the
end of the file. Directives SHALL NOT nest.

#### Scenario: Disabled region passes through

- **WHEN** a region between `fmt off` and `fmt on` is formatted
- **THEN** its bytes are identical before and after

#### Scenario: Directive inside a disabled region

- **WHEN** a second `fmt off` appears inside a disabled region
- **THEN** it is treated as an ordinary comment, not a nested directive

### Requirement: Formatting is parse-stable

Formatted output SHALL re-parse to a program equivalent to the input's
program: the formatter SHALL change only whitespace, comments' positions,
and line structure — never tokens, order, or semantics. An unformattable
document SHALL NOT be partially rewritten.

#### Scenario: Formatted code re-parses identically

- **WHEN** formatted output is lexed and parsed again
- **THEN** it produces the same program structure as the original

#### Scenario: Sugar is never expanded

- **WHEN** the input uses sugared constructs such as promoted constructor
  parameters or a constructor-less class
- **THEN** the output keeps the sugared form without synthesized members
