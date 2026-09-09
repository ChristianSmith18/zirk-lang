## MODIFIED Requirements

### Requirement: Reserved words of the whole language

The lexer SHALL recognize as keywords those of the whole language, not only those of the implemented subset. `concurrent` and `spawn` are keywords. `task` and `await` are NOT keywords (removed from the language; the parser treats them in a construct position as removed constructs). `select`, `scope`, `shield` are ordinary identifiers.

#### Scenario: Keyword outside the subset
- **WHEN** `class`, `for`, `match`, `concurrent`, or `spawn` is tokenized
- **THEN** the corresponding keyword token is produced and no identifier is produced

#### Scenario: Removed word is an identifier
- **WHEN** `mut task = 1;` is tokenized
- **THEN** `task` is produced as an identifier
