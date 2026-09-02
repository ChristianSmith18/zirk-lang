## MODIFIED Requirements

### Requirement: Subset types

The checker SHALL support the types `Void`, `Int32`, `Boolean`, `String`, nullable types (`T?`), and `enum` without associated data, per `ZIRK_LANGUAGE_SPEC.md` sections 3 and 4.

#### Scenario: Unknown type
- **WHEN** an annotation names a type outside the subset
- **THEN** a diagnostic naming the type is emitted
- **AND** if the type exists in the full language, the help notes that it is not implemented yet

#### Scenario: `T?` is distinct from `T`
- **WHEN** the type `String?` is compared with `String`
- **THEN** the checker treats them as distinct types, not interchangeable without coalescing or safe access

### Requirement: Return consistency

The checker SHALL verify that the returned value matches the declared return type, and that every path of a non-`Void` function returns a value, considering that `if`, `match`, and blocks ending in an infinite loop (`loop` with no exiting `break`) may be part of that path.

#### Scenario: Return of the wrong type
- **WHEN** a function declared as `Int32` returns a string
- **THEN** a diagnostic pointing at the returned expression is emitted

#### Scenario: Path without a return
- **WHEN** a non-`Void` function has an execution path that ends without returning
- **THEN** a diagnostic pointing at the end of that path is emitted

#### Scenario: Return with a value in a Void function
- **WHEN** a `Void` function returns a value
- **THEN** a diagnostic pointing at the expression is emitted

#### Scenario: Every `if` branch returns
- **WHEN** a non-`Void` function ends in an `if`/`else` where both branches return
- **THEN** checking succeeds without requiring an additional return afterward

## ADDED Requirements

### Requirement: Typing of loops and of `break`/`continue`

The checker SHALL require the condition of `for` and `while` to be `Boolean`, with no truthiness, and SHALL reject `break`/`continue` outside a loop.

#### Scenario: Non-boolean condition
- **WHEN** `while 1 { }` is written
- **THEN** a diagnostic stating that the condition must be `Boolean` is emitted

#### Scenario: `break` outside a loop
- **WHEN** `break` appears outside any loop, even inside a nested function
- **THEN** a diagnostic pointing at the `break` is emitted

### Requirement: `for ... in` over the minimal iteration protocol

The checker SHALL admit `for ... in` over ranges (`0..N`, `0..=N`) and over `String` iterated by character. Over any other type, it SHALL reject it, stating that iteration over user-defined types arrives with Phase 3's traits.

#### Scenario: Iteration over a range
- **WHEN** `for i in 0..10 { }` is written
- **THEN** `i` has type `Int32` inside the body

#### Scenario: Iteration over an unsupported type
- **WHEN** `for x in value { }` is written and `value` is neither a range nor `String`
- **THEN** a diagnostic stating that this type is not iterable yet is emitted

### Requirement: `if` as an expression requires compatible branches

The checker SHALL admit `if`/`else` in expression position only when both branches are present and produce compatible types. In any other case, `if` is valid only as a statement.

#### Scenario: Compatible branches
- **WHEN** `if x > 0 { "positive" } else { "not positive" }` is evaluated in expression position
- **THEN** the resulting type is `String`

#### Scenario: Missing `else` branch in expression position
- **WHEN** an `if` without `else` is used where a value is expected
- **THEN** a diagnostic stating that the alternative branch is missing is emitted

#### Scenario: Branches with incompatible types
- **WHEN** the branches of an `if` used as an expression produce distinct, unrelated types
- **THEN** a diagnostic pointing at both types is emitted

### Requirement: Typing of optional, named, variadic parameters and default values

The checker SHALL verify that every call resolves to a valid assignment of arguments to parameters: named ones are matched by name, missing ones with a default value take it from the signature, and extras are grouped into the variadic parameter if one exists.

#### Scenario: Optional parameter not provided
- **WHEN** `greet()` is called with `name?: String` and no argument
- **THEN** `name` has value `null` inside the body

#### Scenario: Nonexistent named argument
- **WHEN** a call names an argument that does not exist in the signature
- **THEN** a diagnostic naming the unknown parameter is emitted

#### Scenario: Type of the variadic
- **WHEN** `sum(1, 2, 3)` is called with `...values: Int32`
- **THEN** `values` has the sequence type of `Int32` inside the body

### Requirement: Typing of closures and immutable capture

The checker SHALL infer a lambda's type from its parameters and its body, SHALL record which variables of the enclosing scope it captures, and SHALL reject mutation of a captured variable inside the closure's body.

#### Scenario: Type of a lambda
- **WHEN** `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;` is declared
- **THEN** `ADD` has function type from `(Int32, Int32)` to `Int32`

#### Scenario: Capture of an external variable
- **WHEN** a lambda references a variable declared in the scope that contains it
- **THEN** checking succeeds and the variable is recorded as captured

#### Scenario: Mutation of a captured variable
- **WHEN** a lambda's body attempts to reassign a captured variable from the enclosing scope
- **THEN** a diagnostic stating that the capture is immutable is emitted

### Requirement: `match` exhaustiveness

The checker SHALL require every `match` over an `enum` to cover all of its constructors, or include the `_` wildcard. `match` over types without a closed set of values SHALL require the `_` wildcard as its final arm.

#### Scenario: `enum` covered completely
- **WHEN** a `match` over `Direction` has an arm for each of its four constructors
- **THEN** checking succeeds without requiring `_`

#### Scenario: Incomplete `enum` without a wildcard
- **WHEN** a `match` over `Direction` covers only two of its four constructors and has no `_`
- **THEN** a diagnostic naming the missing constructors is emitted

#### Scenario: `match` over `Int32` without a final wildcard
- **WHEN** a `match` over an `Int32` value does not end with a `_` arm
- **THEN** a diagnostic stating that the wildcard is mandatory for that type is emitted

#### Scenario: Type of `match` as an expression
- **WHEN** every arm of a `match` used as an expression produces the same type
- **THEN** that is the `match`'s type

### Requirement: Null coalescing

The checker SHALL require both operands of `??` to share a common type, producing the non-nullable type when the right operand is not nullable.

#### Scenario: Coalescing with a non-nullable fallback
- **WHEN** `name ?? "anonymous"` is evaluated with `name: String?`
- **THEN** the result has type `String`

#### Scenario: Coalescing with a nullable fallback
- **WHEN** both operands of `??` are nullable
- **THEN** the result remains nullable

#### Scenario: `??` over a non-nullable left operand
- **WHEN** `??` is used over an expression of non-nullable type
- **THEN** a diagnostic stating that the operator is unnecessary is emitted

#### Scenario: Operands without a common type
- **WHEN** the operands of `??` do not share a common type
- **THEN** a diagnostic pointing at both types is emitted

### Requirement: Assignment between nullable and non-nullable types

The checker SHALL admit assigning a value of type `T` where `T?` is expected, and SHALL reject the opposite direction without explicit coalescing.

#### Scenario: Widening to nullable
- **WHEN** a `String` is assigned to a variable declared `String?`
- **THEN** checking succeeds

#### Scenario: Narrowing without coalescing
- **WHEN** a `String?` is assigned to a variable declared `String`
- **THEN** a diagnostic is emitted
- **AND** the help suggests `??` to supply a default value

#### Scenario: `null` as a value
- **WHEN** `null` is assigned to a variable of a non-nullable type
- **THEN** a diagnostic stating that the type does not admit absence of value is emitted

### Requirement: `?.` deferred to the objects phase

The checker SHALL reject `?.` with the not-implemented-construct diagnostic, pointing at Phase 3.

The safe-access operator requires a member to access, and no type in this phase has members: classes, records, and traits are Phase 3. See the design's decision D8.

#### Scenario: Use of `?.`
- **WHEN** `user?.name` is written
- **THEN** a diagnostic naming the operator is emitted
- **AND** it states that it arrives in Phase 3, together with the types that have members
- **AND** it is NOT reported as an unexpected token
