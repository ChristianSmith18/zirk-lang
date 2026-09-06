# Delta spec: zirk-ir-lowering

## ADDED Requirements

### Requirement: String repetition is a block-opening operation

`String * Int` SHALL be recognized as a block-opening expression in the same sense as `if`/`match`/`?.`/calls: its negative-count guard emits a `fail`/`cont` split, so every operand evaluated before the repetition — a sibling operand of an enclosing binary, a call argument, a field receiver — SHALL be held through a slot and reloaded on the continuation side rather than referenced across the branch.

#### Scenario: Repeat inside a concatenation
- **WHEN** `"x: " + "ab" * n` is lowered
- **THEN** the left operand is reloaded in the continuation block and the verifier accepts the function

#### Scenario: Repeat inside a call argument
- **WHEN** `f("a" * 3, other)` is lowered and `other` was computed first
- **THEN** `other` survives the guard branch through a slot and the verifier accepts the function

#### Scenario: Standalone repeat stays valid
- **WHEN** `stdout.println("ab" * 3)` is lowered as a whole statement
- **THEN** the verifier accepts the function and the printed value is `"ababab"`
