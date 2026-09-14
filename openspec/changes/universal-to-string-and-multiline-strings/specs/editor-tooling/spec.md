# editor-tooling Delta

## MODIFIED Requirements

### Requirement: Language configuration

The editor SHALL know the language's comment delimiters and its
opening/closing pairs, including the `'''…'''` triple-quoted string
delimiters.

#### Scenario: Commenting a selection

- **WHEN** the comment command is used
- **THEN** the `//` or `/* */` delimiters from `ZIRK_LANGUAGE_SPEC.md` section 1 are inserted

#### Scenario: Auto-closing

- **WHEN** `{`, `(`, `[`, or a quote is typed
- **THEN** the editor offers the corresponding closing character

#### Scenario: Triple-quote highlighting

- **WHEN** a `'''…'''` literal is opened
- **THEN** its content is scoped as a string and `{expr}` interpolations
  inside it keep their expression scopes

### Requirement: The extension's formatter is provisional

The extension's own formatter MUST remain provisional while `zirk format`
does not cover the full style surface, and MUST NOT be presented as the
definition of Zirk's style.

`ZIRK_COMPILER_SPEC.md` section 10 defines the official formatter as
canonical, idempotent, and free of configuration that would fragment the
style. An editor formatter that respects user configuration does not meet
that contract. For the triple-quoted-string layout rule specifically, the
extension SHALL delegate to `zirk format` once it ships.

#### Scenario: Provisional nature documented

- **WHEN** the extension's documentation is consulted
- **THEN** it states that its formatter is provisional and that the
  canonical one is `zirk format`

#### Scenario: Delegation for delivered rules

- **WHEN** the extension formats a file containing `'''` literals and
  `zirk format` is available
- **THEN** the `'''` layout is produced by `zirk format`, not by editor-side
  logic

## ADDED Requirements

### Requirement: Triple-quote edge guide

The extension SHALL draw a column-level visual guide joining the opening
and closing `'''` delimiters of a triple-quoted literal, so the literal's
left edge is visible while editing — the same class of affordance as
indent guides or bracket-pair guides in other languages. The guide SHALL be
implemented as an editor decoration painted at the delimiters' column for
every line the literal spans, independent of optional editor settings.

#### Scenario: Guide renders the edge

- **WHEN** a `'''` literal occupies several lines
- **THEN** a vertical line connects the opening and closing delimiters at
  their shared column

#### Scenario: Guide tracks edits

- **WHEN** the delimiters' column changes or lines are added inside the
  literal
- **THEN** the guide repaints to match the current edge
