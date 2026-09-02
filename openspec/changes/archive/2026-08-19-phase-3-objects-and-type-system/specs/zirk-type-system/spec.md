## MODIFIED Requirements

### Requirement: `?.` is deferred to the objects phase

The checker SHALL type `expr?.member` as the member's type in its nullable form when the receiver is nullable.

The previous phase's deferral ends here: its reason was that no type had members.

#### Scenario: Safe access on a nullable value
- **WHEN** `usuario` has type `User?` and `nombre` is `String`
- **THEN** `usuario?.nombre` has type `String?`

#### Scenario: `?.` on a non-nullable value
- **WHEN** `?.` is used on an expression that is never null
- **THEN** a diagnostic stating that the operator is unnecessary is emitted, with `.` as a suggestion

#### Scenario: Nonexistent member
- **WHEN** `?.` names a member the type does not have
- **THEN** a diagnostic naming the member and the type is emitted

## ADDED Requirements

### Requirement: Nominal types and subtyping

The checker SHALL treat every class, record, value class, and enum as a distinct nominal type, and SHALL admit a value where one of its superclasses or an implemented contract is expected.

#### Scenario: Subclass where the base is expected
- **WHEN** an instance of `Admin` is passed to a parameter of type `User`
- **THEN** the check succeeds

#### Scenario: Implementation where the contract is expected
- **WHEN** an instance is passed to a parameter whose type is a contract it implements
- **THEN** the check succeeds

#### Scenario: Two types with the same shape are not the same
- **WHEN** two classes declare the same fields and one is assigned where the other is expected
- **THEN** a diagnostic is emitted: equivalence is by name, not by shape

#### Scenario: Base where the subclass is expected
- **WHEN** an instance of `User` is passed to a parameter of type `Admin`
- **THEN** a diagnostic is emitted

### Requirement: Member resolution

The checker SHALL resolve an access `expr.member` against `expr`'s type and its inheritance chain, respecting visibility.

#### Scenario: Inherited member
- **WHEN** a field declared in the superclass is accessed
- **THEN** it resolves to that declaration

#### Scenario: Nonexistent member
- **WHEN** a member no ancestor declares is accessed
- **THEN** a diagnostic naming the member and the type is emitted

#### Scenario: Member hidden by visibility
- **WHEN** a `private` member is accessed from outside
- **THEN** a visibility diagnostic, distinct from the nonexistent-member one, is emitted

### Requirement: Shadowing and capture qualification

The checker SHALL reject a local declaration that hides another still-visible local or parameter. A lambda parameter MAY share a capture's name only when the capture is referenced as `this.name`; the bare name designates the parameter.

#### Scenario: Duplicate local in a nested scope
- **WHEN** an inner block declares a local name that is still visible
- **THEN** a diagnostic pointing at both declarations is emitted

#### Scenario: Parameter and capture collision
- **WHEN** a lambda declares the parameter `prefix` and reads `this.prefix`
- **THEN** `prefix` resolves to the parameter and `this.prefix` to the outer capture

### Requirement: Object reference strictness

On class references, `mut` SHALL allow reassigning and mutating the object,
`inmut` SHALL only prevent reassignment, and `inmut::strict` SHALL prevent
reachable mutation. A strict reference SHALL NOT become a mutable alias nor
be acquired while a mutable alias remains accessible.

#### Scenario: Mutable clone from a strict reference
- **WHEN** a strict object implements `Cloneable` and is cloned into a `mut` binding
- **THEN** the independent clone can be mutated without altering the original object

### Requirement: Constructor resolution

The checker SHALL select among multiple `construct` by arity, types, optionals, and names, SHALL allow reordering named arguments, and SHALL reject duplicate effective signatures or ambiguous calls.

#### Scenario: Reordered named construction
- **WHEN** `User(name: "Cristian", id: 1)` matches `construct(id: UInt64, name: String)`
- **THEN** that signature is selected and each value is bound by name

### Requirement: Checkable casts

A cast to a related type SHALL be checked at runtime and fail in a controlled way; a cast between unrelated types SHALL be rejected at compile time.

#### Scenario: Valid downcast
- **WHEN** a `User` that is actually an `Admin` is converted to `Admin`
- **THEN** the result is the instance

#### Scenario: Invalid downcast
- **WHEN** the value is not of the requested type
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

#### Scenario: Unrelated types
- **WHEN** converting between two types that share neither hierarchy nor contract
- **THEN** a diagnostic is emitted at compile time

### Requirement: Operators resolve by contract

The checker SHALL resolve an operator by looking up the contract its type implements, instead of comparing against a fixed list of types.

#### Scenario: Concatenation
- **WHEN** `"a" + "b"` is evaluated
- **THEN** the resulting type is `String`

#### Scenario: Operand without the contract
- **WHEN** an operand does not implement the operator's contract
- **THEN** the diagnostic names the missing contract, not a list of admitted types

### Requirement: The final function-type syntax is not delivered in Phase 3

The Phase 3 checker SHALL reject any position that requires annotating a
closure's type —parameter, return, or attribute— with a diagnostic stating
that `Function(P...) => R` / `Fn(P...) => R` arrives in a later phase.

This is an implementation limit, not the final semantics. The language
already defines signature compatibility, escapable closures, and automatic
storage per `docs/CORE_LANGUAGE_SEMANTICS.md`.

#### Scenario: Closure in a local variable
- **WHEN** `inmut F = (a: Int32): Int32 => a + 1;` is declared and `F(1)` is called
- **THEN** the check succeeds
- **AND** `F`'s type is inferred without being written

#### Scenario: Closure as a parameter type
- **WHEN** a function declares a parameter whose type is meant to be a function
- **THEN** a diagnostic stating that function types arrive in a later phase is emitted
- **AND** it is NOT reported as an unknown type

#### Scenario: Closure as a return type
- **WHEN** a function declares that it returns a closure
- **THEN** the same diagnostic is emitted

#### Scenario: Closure as an attribute type
- **WHEN** a class declares an attribute whose type is meant to be a function
- **THEN** the same diagnostic is emitted

#### Scenario: Returning a locally created closure
- **WHEN** a function constructs a closure and returns it
- **THEN** a diagnostic stating that a closure cannot escape the function that creates it is emitted
