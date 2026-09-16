## MODIFIED Requirements

### Requirement: Formatting

The server SHALL answer `textDocument/formatting` with edits equivalent to
`zirk format` applied to the in-memory text — the full canonical document
formatting, not only triple-quoted-string layout. When the text cannot be
formatted, the server SHALL return no edits.

#### Scenario: Formatting normalizes layout

- **WHEN** the user requests formatting on a document whose layout
  deviates from canonical form
- **THEN** the server returns edits producing the canonical form

#### Scenario: Unformattable text

- **WHEN** the in-memory text cannot be formatted
- **THEN** the server returns an empty result and the document is left
  unchanged
