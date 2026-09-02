## ADDED Requirements

### Requirement: Function declaration

The parser SHALL recognize function declarations with the form from `ZIRK_LANGUAGE_SPEC.md` section 6: `fn` name, list of typed parameters, return type after `:`, and body in braces.

#### Scenario: Function without parameters
- **WHEN** `fn main(): Void { }` is parsed
- **THEN** a function declaration named `main`, with no parameters and return type `Void`, is produced

#### Scenario: Function with parameters
- **WHEN** `fn add(a: Int32, b: Int32): Int32 { return a + b; }` is parsed
- **THEN** a function with two typed parameters and return type `Int32` is produced

#### Scenario: Missing return type
- **WHEN** a function is declared without a return type
- **THEN** a diagnostic is emitted pointing to where the type was expected
- **AND** the help indicates that the return type is mandatory at this phase

### Requirement: Variable declaration

The parser SHALL recognize declarations with `mut` and `inmut`, with optional type annotation when there is an initializer.

#### Scenario: Variable with explicit type
- **WHEN** `mut count: Int32 = 0;` is parsed
- **THEN** a mutable declaration with type `Int32` and an initializer is produced

#### Scenario: Variable with inferred type
- **WHEN** `mut count = 0;` is parsed
- **THEN** a mutable declaration with no type annotation is produced

#### Scenario: Declaration without initializer or type
- **WHEN** `mut count;` is parsed
- **THEN** a diagnostic is emitted indicating that the type or the initializer is missing

### Requirement: Expressions and precedence

The parser SHALL build expressions respecting the conventional precedence and associativity of the operators from `ZIRK_LANGUAGE_SPEC.md` section 4.

From highest to lowest precedence: unary (`!`, `-`); multiplicative (`*`, `/`, `%`); additive (`+`, `-`); comparison (`<`, `<=`, `>`, `>=`); equality (`==`, `!=`); conjunction (`&&`); disjunction (`||`).

#### Scenario: Multiplicative precedence over additive
- **WHEN** `1 + 2 * 3` is parsed
- **THEN** the tree represents `1 + (2 * 3)`

#### Scenario: Left associativity
- **WHEN** `10 - 4 - 3` is parsed
- **THEN** the tree represents `(10 - 4) - 3`

#### Scenario: Parentheses alter precedence
- **WHEN** `(1 + 2) * 3` is parsed
- **THEN** the tree represents the addition as the left operand of the product

#### Scenario: Conjunction over disjunction
- **WHEN** `a || b && c` is parsed
- **THEN** the tree represents `a || (b && c)`

### Requirement: Conditional statement

The parser SHALL recognize `if` and `else` as a statement, with bodies always in braces.

At this phase `if` is **not** admitted as an expression, even though the spec allows it: that is Phase 2.

#### Scenario: Simple conditional
- **WHEN** `if x > 0 { }` is parsed
- **THEN** a conditional statement with no alternative branch is produced

#### Scenario: Conditional with an alternative
- **WHEN** `if x > 0 { } else { }` is parsed
- **THEN** a conditional statement with both branches is produced

#### Scenario: Chaining
- **WHEN** `if a { } else if b { } else { }` is parsed
- **THEN** a conditional whose alternative branch is another conditional is produced

#### Scenario: Body without braces
- **WHEN** `if x > 0 return;` is parsed
- **THEN** a diagnostic is emitted indicating that the body must be enclosed in braces

### Requirement: Function calls and return

The parser SHALL recognize calls with positional arguments and the `return` statement.

#### Scenario: Call with arguments
- **WHEN** `add(1, 2)` is parsed
- **THEN** a call with two arguments is produced

#### Scenario: Return with a value
- **WHEN** `return a + b;` is parsed
- **THEN** a return whose expression is the sum is produced

#### Scenario: Return without a value
- **WHEN** `return;` is parsed
- **THEN** a return with no expression is produced

### Requirement: Optional semicolon

The parser SHALL allow omitting the semicolon when there is no ambiguity, per `ZIRK_LANGUAGE_SPEC.md` section 1.

#### Scenario: Statements without semicolons
- **WHEN** statements separated by line breaks and without `;` are parsed
- **THEN** the resulting tree is equivalent to that of the same statements with `;`

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not implemented, distinguishing them from syntax errors.

#### Scenario: Construct from a later phase
- **WHEN** `class`, `for`, `while`, `loop`, `match`, `try`, `task`, `parallel`, or `thread` is parsed
- **THEN** the diagnostic SHALL name the construct
- **AND** SHALL indicate that it is not implemented yet
- **AND** SHALL NOT be reported as an unexpected token

#### Scenario: Module import
- **WHEN** an `import` statement is parsed
- **THEN** the diagnostic indicates that multi-file modules arrive in a later phase

### Requirement: Location on every node

Every node in the tree SHALL expose the source span that originated it.

A node without a location cannot produce the diagnostic that `ZIRK_COMPILER_SPEC.md` section 8 requires.

#### Scenario: Span of any node
- **WHEN** any node of the produced tree is inspected
- **THEN** it exposes the start and end location in the source
