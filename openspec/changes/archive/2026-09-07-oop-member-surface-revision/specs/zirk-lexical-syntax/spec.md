# zirk-lexical-syntax Delta Spec

## ADDED Requirements

### Requirement: Member marker token `#`
The lexer SHALL recognize `#` as its own token, used to introduce member markers such as `#override`. `#` SHALL NOT combine with the following identifier into a single token; the marker name is a separate identifier or keyword.

#### Scenario: Marker token
- **WHEN** `#override` is tokenized
- **THEN** a `#` token followed by the `override` keyword is produced

#### Scenario: `#` away from a member
- **WHEN** `#` appears where no member marker is legal
- **THEN** the token is produced and the parser emits the corresponding diagnostic

### Requirement: `final` is a reserved word
The lexer SHALL recognize `final` as a keyword of the whole language, so it can be used as the class/member sealing modifier and produce targeted diagnostics elsewhere.

#### Scenario: Final keyword
- **WHEN** `final` is tokenized
- **THEN** a keyword token is produced and no identifier is produced
