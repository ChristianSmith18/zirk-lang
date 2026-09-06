## MODIFIED Requirements

### Requirement: Expressions and precedence

The parser SHALL build expressions respecting the conventional precedence and associativity of the operators from `ZIRK_LANGUAGE_SPEC.md` section 4.

From highest to lowest precedence: exponentiation (`**`, right-associative); unary (`!`, `-`); multiplicative (`*`, `/`, `%`); additive (`+`, `-`); comparison (`<`, `<=`, `>`, `>=`); equality (`==`, `!=`); conjunction (`&&`); disjunction (`||`).

`**` SHALL be parsed as an infix operator in the implemented subset, not
rejected as a deferred construct. Its right operand MAY be a unary expression,
so `2 ** -1` parses with the negation as the right operand.

#### Scenario: Multiplicative precedence over additive
- **WHEN** `1 + 2 * 3` is parsed
- **THEN** the tree represents `1 + (2 * 3)`

#### Scenario: Left associativity
- **WHEN** `10 - 4 - 3` is parsed
- **THEN** the tree represents `(10 - 4) - 3`

#### Scenario: Parentheses alter precedence
- **WHEN** `(1 + 2) * 3` is parsed
- **THEN** the tree represents the sum as the left operand of the product

#### Scenario: Conjunction over disjunction
- **WHEN** `a || b && c` is parsed
- **THEN** the tree represents `a || (b && c)`

#### Scenario: Exponentiation is right-associative
- **WHEN** `2 ** 3 ** 2` is parsed
- **THEN** the tree represents `2 ** (3 ** 2)`

#### Scenario: Exponentiation binds tighter than unary minus
- **WHEN** `-2 ** 2` is parsed
- **THEN** the tree represents `-(2 ** 2)`

#### Scenario: Exponentiation binds tighter than multiplication
- **WHEN** `3 * 2 ** 2` is parsed
- **THEN** the tree represents `3 * (2 ** 2)`
