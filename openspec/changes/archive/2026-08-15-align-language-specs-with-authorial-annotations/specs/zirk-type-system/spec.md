## ADDED Requirements

### Requirement: Shadowing and capture qualification
The checker SHALL reject a local declaration that hides a still-visible local or parameter. A lambda parameter MAY share a captured outer name only when the capture is addressed as `this.name`; the plain name SHALL denote the lambda-local binding.

#### Scenario: Nested local shadowing
- **WHEN** a nested block declares `label` while an outer local `label` remains visible
- **THEN** a compile-time diagnostic names the existing binding

#### Scenario: Qualified capture collision
- **WHEN** a lambda parameter is `prefix` and its body reads `this.prefix`
- **THEN** `prefix` resolves to the parameter and `this.prefix` resolves to the captured outer value

### Requirement: Parameter collection semantics
Every optional parameter SHALL retain an explicit type and SHALL follow required positional parameters. A variadic parameter SHALL be an ordered read-only `Iterable<T>` for the duration of the call.

#### Scenario: Untyped optional parameter
- **WHEN** a declaration contains `prefix?` without `: Type`
- **THEN** the checker emits a missing parameter type diagnostic

#### Scenario: Variadic iteration
- **WHEN** a variadic `...values: String` is used in `for value in values`
- **THEN** `value` has type `String`

### Requirement: Class defaults and constructor resolution
An unmodified class field SHALL be `public mut`. A class MAY declare multiple constructors with distinct effective signatures. Selection SHALL consider arity, types, optional parameters, and names and SHALL reject ambiguous or duplicate effective signatures.

#### Scenario: Default field modifiers
- **WHEN** a class declares `name: String;`
- **THEN** the member has the same access and mutability as `public mut name: String;`

#### Scenario: Reordered named construction
- **WHEN** `User(name: "Cristian", id: 1)` matches `construct(id: UInt64, name: String)`
- **THEN** the constructor is selected by labels and each value binds to its named parameter

### Requirement: Reserved operator methods
User-defined types SHALL implement language operator contracts through reserved methods including `_add` and `_subtract` in safe code. Native types SHALL NOT be reopened by application code. Operator methods SHALL NOT change operator precedence, arity, or evaluation category.

#### Scenario: User type addition
- **WHEN** a class implements a valid `_add(other: Self): Self`
- **THEN** `left + right` resolves to that implementation

#### Scenario: Native type replacement
- **WHEN** application code attempts to replace `_add` on `String` or a numeric native type
- **THEN** the checker rejects reopening the native type

### Requirement: Value, enum, array, iteration, and generator semantics
Records SHALL be immutable with structural field equality; value classes SHALL be distinct domain types without observable identity and SHALL be storable inline; an unmapped traditional enum case SHALL expose its exact case name as its default string value and no implicit numeric index; all arrays SHALL have fixed length; `String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and `Iterable<T>` while preserving locals between yields.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both values retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: String iteration
- **WHEN** a string is consumed by `for ... in`
- **THEN** iteration produces the string's public character units in order

### Requirement: Bound callable cloning and pipelines
`clone(receiver.method)` SHALL produce a local callable bound to the same receiver and preserving the method's parameters, result, effects, errors, permissions, and safety contract without cloning the receiver. The pipe operator SHALL pass its left value into the next ordinary function and SHALL NOT require that function to be a method of the value's class.

#### Scenario: Cloned stdout method
- **WHEN** `inmut print = clone(stdout.println)` is followed by `print("hello")`
- **THEN** the call invokes `println` on the original `stdout` receiver

#### Scenario: Pure-function pipeline
- **WHEN** `users |> filter(is_active) |> map(to_summary)` is checked
- **THEN** each stage is checked as an ordinary typed function receiving the previous stage's result
