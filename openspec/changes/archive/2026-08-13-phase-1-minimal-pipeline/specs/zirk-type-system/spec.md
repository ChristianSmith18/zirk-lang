## ADDED Requirements

### Requirement: Subset types

The checker SHALL support the types `Void`, `Int32`, `Boolean`, and `String`, per `ZIRK_LANGUAGE_SPEC.md` section 3.

#### Scenario: Unknown type
- **WHEN** an annotation names a type that does not belong to the subset
- **THEN** a diagnostic naming the type is emitted
- **AND** if the type exists in the full language, the help indicates that it is not implemented yet

### Requirement: Absence of implicit conversions

The checker SHALL NOT insert implicit conversions between distinct types. Per `ZIRK_LANGUAGE_SPEC.md` section 3, conversions that may lose information are not implicit.

#### Scenario: Incompatible type assignment
- **WHEN** `mut total: Int32 = "cuarenta";` is declared
- **THEN** a diagnostic is emitted pointing to the initializer
- **AND** the cause indicates that there is no implicit conversion from `String` to `Int32`

#### Scenario: Operands of distinct types
- **WHEN** an arithmetic operation between `Int32` and `String` is evaluated
- **THEN** a diagnostic pointing to the operation is emitted

### Requirement: Absence of truthiness

The checker SHALL require every condition to be of type `Boolean`. There is no numeric or string truthiness, per `ZIRK_LANGUAGE_SPEC.md` section 3.

#### Scenario: Numeric condition
- **WHEN** `if 1 { }` is written
- **THEN** a diagnostic is emitted indicating that the condition must be `Boolean`

#### Scenario: Boolean condition
- **WHEN** `if x > 0 { }` is written
- **THEN** the check succeeds

### Requirement: Logical operators only over booleans

The `&&`, `||`, and `!` operators SHALL accept only `Boolean` operands.

#### Scenario: Conjunction over integers
- **WHEN** `1 && 2` is evaluated
- **THEN** a diagnostic pointing to the operands is emitted

### Requirement: Mutability

The checker SHALL allow reassignment of `mut` variables and reject it for `inmut` variables, per `ZIRK_LANGUAGE_SPEC.md` section 2.

#### Scenario: Reassignment of a mutable variable
- **WHEN** a variable declared with `mut` is reassigned
- **THEN** the check succeeds

#### Scenario: Reassignment of an immutable variable
- **WHEN** a variable declared with `inmut` is reassigned
- **THEN** a diagnostic pointing to the reassignment is emitted
- **AND** the help suggests declaring it with `mut` if it must change

### Requirement: Unambiguous inference

The checker SHALL infer the type of a variable with no annotation from its initializer, when inference is unambiguous.

#### Scenario: Inference from an integer literal
- **WHEN** `mut count = 0;` is declared
- **THEN** the inferred type is `Int32`

#### Scenario: Inference from a string literal
- **WHEN** `mut name = "Zirk";` is declared
- **THEN** the inferred type is `String`

### Requirement: Name resolution

The checker SHALL resolve each identifier to its declaration, respecting the block and function scopes from `ZIRK_LANGUAGE_SPEC.md` section 2.

#### Scenario: Undeclared identifier
- **WHEN** an identifier that has not been declared is used
- **THEN** a diagnostic naming it and pointing to its use is emitted

#### Scenario: Variable outside its scope
- **WHEN** a variable declared inside a block is used outside that block
- **THEN** an undeclared-identifier diagnostic is emitted

#### Scenario: Shadowing in an inner block
- **WHEN** an inner block declares a variable with the name of an outer one
- **THEN** uses inside the block resolve to the inner declaration

### Requirement: Use before availability

Flow analysis SHALL prevent reading a variable that does not yet have a value, per `ZIRK_LANGUAGE_SPEC.md` section 2.

#### Scenario: Read before assignment
- **WHEN** a variable declared without an initializer and not yet assigned is read
- **THEN** a diagnostic pointing to the read is emitted

### Requirement: Signature matching in calls

The checker SHALL verify the arity and types of each call's arguments against the function's signature.

#### Scenario: Incorrect number of arguments
- **WHEN** a call passes more or fewer arguments than declared
- **THEN** a diagnostic indicating the expected and received count is emitted

#### Scenario: Incorrect argument type
- **WHEN** an argument does not match the parameter's type
- **THEN** a diagnostic pointing to that argument is emitted

### Requirement: Return coherence

The checker SHALL verify that the returned value matches the declared return type, and that every path of a non-`Void` function returns a value.

#### Scenario: Return of incorrect type
- **WHEN** a function declared `Int32` returns a string
- **THEN** a diagnostic pointing to the returned expression is emitted

#### Scenario: Path without a return
- **WHEN** a non-`Void` function has an execution path that ends without returning
- **THEN** a diagnostic pointing to the end of that path is emitted

#### Scenario: Return with a value in a Void function
- **WHEN** a `Void` function returns a value
- **THEN** a diagnostic pointing to the expression is emitted

### Requirement: Integer literal overflow

The checker SHALL reject integer literals that do not fit their type, per the `ZIRK_LANGUAGE_SPEC.md` section 3 rule that ordinary overflow produces a controlled error.

#### Scenario: Out-of-range literal
- **WHEN** a literal greater than its maximum value is assigned to an `Int32`
- **THEN** a diagnostic indicating the allowed range is emitted

### Requirement: Entrypoint

The checker SHALL require the existence of a `main` function with the signature `fn main(): Void`, per `ZIRK_RUNTIME_SPEC.md` section 2.

#### Scenario: Missing entrypoint
- **WHEN** the compiled file does not declare `main`
- **THEN** a diagnostic indicating that the entrypoint is missing is emitted

#### Scenario: Entrypoint with an incorrect signature
- **WHEN** `main` is declared with parameters or with a return type other than `Void`
- **THEN** a diagnostic indicating the expected signature is emitted
