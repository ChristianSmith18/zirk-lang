## ADDED Requirements

### Requirement: Index expression grammar

The parser SHALL recognize `expr '[' expr ']'` as a postfix index expression at the same precedence tier as method call and field access, left-associative and chainable, valid both as an ordinary read expression and, when the receiver type permits mutation, as a simultaneous-assignment or ordinary assignment target. The grammar itself SHALL NOT restrict which receiver types support indexing — that restriction belongs to the checker.

#### Scenario: Index expression parses as a place
- **WHEN** `view[0] = 0x7f;` is parsed
- **THEN** it produces an index expression usable as an assignment destination, the same classification `expr.field` already receives

#### Scenario: Chained index and field access
- **WHEN** `a.field[i][j]` is parsed
- **THEN** it produces a left-associative chain of field access followed by two index expressions

#### Scenario: Indexing an unsupported receiver type is a checker error, not a parse error
- **WHEN** an expression whose type does not support indexing is subscripted
- **THEN** parsing succeeds and the checker rejects the expression, naming the receiver type
