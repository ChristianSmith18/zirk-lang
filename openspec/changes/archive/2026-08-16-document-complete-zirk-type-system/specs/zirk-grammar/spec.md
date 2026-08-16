## ADDED Requirements

### Requirement: Contextual constructor expressions
The grammar SHALL retain the complete contained operator tree of `Float(expression)` and `String(expression)` so semantic analysis can apply explicit deep contextual conversion before evaluating compatible contained arithmetic or concatenation operators.

#### Scenario: Nested contextual arithmetic
- **WHEN** `Float((a + 1) / (b * 2))` is parsed
- **THEN** the constructor contains the entire nested arithmetic tree rather than an already-evaluated integer result

### Requirement: Native String repetition syntax
The grammar SHALL accept multiplication and compound multiplication between a String expression and an integer expression, leaving type checking to enforce operand types, non-negative counts, mutability for `*=`, and allocation bounds.

#### Scenario: Compound String repetition
- **WHEN** `laugh *= 3` is parsed
- **THEN** it is represented as a compound assignment whose semantic result is String repetition

### Requirement: Temporal construction and composition syntax
The grammar SHALL accept ordinary typed constructors, named components, method calls, ISO strings, and the documented temporal operator combinations without introducing a single ambiguous all-purpose Date literal.

#### Scenario: Named Duration construction
- **WHEN** `Duration(hours: 2, minutes: 30)` is parsed
- **THEN** the constructor retains both named temporal components

#### Scenario: Date and Time composition
- **WHEN** `date + time` is parsed
- **THEN** it remains a binary operation for type-directed `DateTime` composition
