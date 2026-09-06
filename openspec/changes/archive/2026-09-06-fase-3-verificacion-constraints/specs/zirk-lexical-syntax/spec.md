# Delta spec: zirk-lexical-syntax

## MODIFIED Requirements

### Requirement: Remaining keywords of the whole language

The lexer SHALL also recognize `do`, `yield`, `interface`, and `trait` as
keywords of the whole language.

`strict` and `value` SHALL NOT be reserved words. `strict` appears only in
the fixed position `inmut::strict`, and `value` has no reserved use;
reserving them would invalidate `mut value = 1;` and
`match r { Ok(value) => ... }`, which are ordinary Zirk and appear in the
spec's own examples.

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
