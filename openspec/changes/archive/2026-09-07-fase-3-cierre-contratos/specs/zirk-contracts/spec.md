# Delta spec: zirk-contracts

## MODIFIED Requirements

### Requirement: Operator contracts

Operator overloading SHALL occur only by implementing the contract the language defines for it, and SHALL NOT alter its precedence or arity, per `ZIRK_LANGUAGE_SPEC.md` section 4.

Contracts use reserved methods: `_add` (`+`), `_subtract` (`-`), `_multiply` (`*`), `_divide` (`/`), `_remainder` (`%`), `_equals` (`==`, and `!=` as its negation), `_less` (`<`), `_less_equal` (`<=`), `_greater` (`>`), and `_greater_equal` (`>=`). User-defined types MAY implement them in safe code; native types SHALL NOT be reopenable from application code.

`String` SHALL implement native concatenation and checked repetition contracts:
`String * Integer` and `Integer * String` return a new String, reject negative
counts, and diagnose unrepresentable allocation sizes.

#### Scenario: String concatenation
- **WHEN** `"a" + "b"` is evaluated
- **THEN** the result is `"ab"`
- **AND** it resolves via the contract that `String` implements, not via a special-cased operator

#### Scenario: Operator on a type that does not implement it
- **WHEN** `+` is applied to a type that does not implement the contract
- **THEN** a diagnostic is emitted naming the missing contract

#### Scenario: Custom type implementing the contract
- **WHEN** a class implements the addition contract and two of its instances are written with `+`
- **THEN** its implementation is invoked

#### Scenario: Comparison through a contract
- **WHEN** a class implements `_less` and two of its instances are written with `<`
- **THEN** its implementation is invoked and the result is `Boolean`

#### Scenario: Inequality from equality
- **WHEN** a class implements `_equals` and two of its instances are written with `!=`
- **THEN** the result is the negation of `_equals`, with no separate method required
