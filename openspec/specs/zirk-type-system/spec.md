# zirk-type-system

## Purpose

Defines the types of the language, name resolution, mutability, inference and flow analysis.

This is where the rules of `ZIRK_LANGUAGE_SPEC.md` that most often surprise someone coming from another language are enforced: no numeric truthiness, no implicit conversions and no function overloading.
## Requirements
### Requirement: Runtime type identity is safe and representation-independent
Every value and declared type MUST expose compiler-provided `Type` identity.
Identity SHALL distinguish constructed generic arguments and provide broad
`TypeKind`, safe names, and declared implements/extends relationships without
exposing layout, private members, or optimized representation. Value type
observation and checked casts SHALL remain separate operations.

#### Scenario: Interface value contains a concrete class
- **WHEN** `value` is declared as an interface and contains an implementing class instance
- **THEN** `value.type()` identifies the concrete class
- **AND** the interface's own kind remains available through `InterfaceName.type()`

#### Scenario: Constructed generic types share optimized code
- **WHEN** two constructed generic types reuse an implementation representation
- **THEN** their `Type` identities remain distinct when their type arguments differ

#### Scenario: A user-declared generic enum instantiates like a compiler-native one
- **WHEN** a program declares `enum Bar<T> { ... }` and instantiates it as `Bar<Int32>`
- **THEN** the instantiation lowers to its own concrete layout, the same mechanism already used for `Result<T,E>`

### Requirement: Structural runtime metadata is explicitly generated
General runtime reflection MUST NOT enumerate members, invoke string-named
methods, access fields dynamically, retain decorators automatically, or expose
compiler syntax. A library requiring runtime structure SHALL generate or author
an ordinary typed descriptor, registry, factory, or accessor that obeys normal
visibility, typing, public API, compatibility, permission, and dead-code rules.

#### Scenario: Framework requires route metadata
- **WHEN** a decorator-backed framework needs routes after compilation
- **THEN** expansion generates an ordinary typed route registry
- **AND** runtime code does not rediscover erased decorator applications

### Requirement: Subset types

The checker SHALL support the types `Void`, `Int32`, `Boolean`, `String`,
nullable types (`T?`), and `enum` without associated data, per
`ZIRK_LANGUAGE_SPEC.md` sections 3 and 4.

The aliases `Int` and `Integer` SHALL resolve to `Int32`, which is exactly
what they name. An alias of an implemented type is not a deferred capability.

#### Scenario: Unknown type
- **WHEN** an annotation names a type that does not belong to the subset
- **THEN** a diagnostic naming the type is emitted
- **AND** if the type exists in the full language, the help indicates that it is not implemented yet

#### Scenario: `T?` is distinct from `T`
- **WHEN** the type `String?` is compared with `String`
- **THEN** the checker treats them as distinct types, not interchangeable without coalescing or safe access

#### Scenario: Short alias for the default integer
- **WHEN** `mut count: Int = 0;` or `mut count: Integer = 0;` is declared
- **THEN** the check succeeds and the type is indistinguishable from `Int32`

#### Scenario: Alias of an unimplemented type
- **WHEN** an annotation names `UInt`
- **THEN** the unimplemented-type diagnostic is emitted with `UInt32`'s phase

### Requirement: Absence of implicit conversions

The checker SHALL NOT insert implicit conversions between distinct non-numeric types or between numeric types that could lose information. Implicit conversions between numeric types are permitted only when the destination can represent every value of the source type exactly, or when the compiler can prove at compile time that the specific value fits. All other conversions between numeric types require an explicit `as` cast.

#### Scenario: Assignment of an incompatible type
- **WHEN** `mut total: Int32 = "cuarenta";` is declared
- **THEN** a diagnostic pointing to the initializer is emitted
- **AND** the cause indicates that there is no implicit conversion from `String` to `Int32`

#### Scenario: Operands of distinct non-numeric types
- **WHEN** `String` and `Int32` are used with `+`
- **THEN** compilation fails because there is no common type between them

#### Scenario: Operands of distinct numeric types with a common type
- **WHEN** `Int8` and `Int32` are used with `+`
- **THEN** the operation is accepted and the result type is `Int32`

#### Scenario: Operands of distinct numeric types with no safe common type
- **WHEN** `UInt64` and `Int64` are used with `+`
- **THEN** compilation fails because no type can represent every value of both exactly

#### Scenario: Narrowing conversion proven safe at compile time
- **WHEN** `mut a: Int8 = 5;` is written with a constant `Int32` literal
- **THEN** the assignment succeeds because `5` is known to fit in `Int8`

#### Scenario: Widening across signedness is safe
- **WHEN** `mut a: Int16 = (5 as UInt8);` is written
- **THEN** the assignment succeeds because every `UInt8` value fits in `Int16`

### Requirement: Absence of truthiness

The checker SHALL require every condition to be of type `Boolean`. There is no numeric or string truthiness, per `ZIRK_LANGUAGE_SPEC.md` section 3.

#### Scenario: Numeric condition
- **WHEN** `if 1 { }` is written
- **THEN** a diagnostic indicating that the condition must be `Boolean` is emitted

#### Scenario: Boolean condition
- **WHEN** `if x > 0 { }` is written
- **THEN** the check succeeds

### Requirement: Logical operators only on Booleans

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

The checker SHALL infer the type of an unannotated variable from its initializer, when the inference is unambiguous.

#### Scenario: Inference from an integer literal
- **WHEN** `mut count = 0;` is declared
- **THEN** the inferred type is `Int32`

#### Scenario: Inference from a string literal
- **WHEN** `mut name = "Zirk";` is declared
- **THEN** the inferred type is `String`

### Requirement: Name resolution

The checker SHALL resolve every identifier to its declaration, respecting the block and function scopes from `ZIRK_LANGUAGE_SPEC.md` section 2.

#### Scenario: Undeclared identifier
- **WHEN** an identifier that has not been declared is used
- **THEN** a diagnostic naming it and pointing to its use is emitted

#### Scenario: Variable outside its scope
- **WHEN** a variable declared inside a block is used outside that block
- **THEN** an undeclared-identifier diagnostic is emitted

#### Scenario: Shadow in an inner block
- **WHEN** an inner block declares a variable with the name of a still-visible outer one
- **THEN** a diagnostic pointing to both declarations is emitted
- **AND** no declaration that silently hides the earlier one is created

### Requirement: Use before availability

Flow analysis SHALL prevent reading a variable that does not yet have a value, per `ZIRK_LANGUAGE_SPEC.md` section 2.

#### Scenario: Read before assignment
- **WHEN** a variable declared without an initializer and not yet assigned is read
- **THEN** a diagnostic pointing to the read is emitted

### Requirement: Signature correspondence in calls

The checker SHALL verify the arity and types of the arguments of every call against the function's signature.

#### Scenario: Incorrect number of arguments
- **WHEN** a call passes more or fewer arguments than declared
- **THEN** a diagnostic indicating the expected and received count is emitted

#### Scenario: Incorrect argument type
- **WHEN** an argument does not match the parameter's type
- **THEN** a diagnostic pointing to that argument is emitted

### Requirement: Return coherence

The checker SHALL verify that the returned value matches the declared return type, and that every path of a non-`Void` function returns a value, considering that `if`, `match`, and blocks terminated by an infinite loop (`loop` without an exiting `break`) may form part of that path.

#### Scenario: Return of an incorrect type
- **WHEN** a function declared `Int32` returns a string
- **THEN** a diagnostic pointing to the returned expression is emitted

#### Scenario: Path without a return
- **WHEN** a non-`Void` function has an execution path that ends without returning
- **THEN** a diagnostic pointing to the end of that path is emitted

#### Scenario: Return with a value in a Void function
- **WHEN** a `Void` function returns a value
- **THEN** a diagnostic pointing to the expression is emitted

#### Scenario: Every `if` branch returns
- **WHEN** a non-`Void` function ends in an `if`/`else` where both branches return
- **THEN** the check succeeds without requiring an additional return afterward

### Requirement: Overflow of integer literals

The checker SHALL reject integer literals that do not fit their type, per the `ZIRK_LANGUAGE_SPEC.md` section 3 rule that ordinary overflow produces a controlled error.

#### Scenario: Literal out of range
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

### Requirement: Typing of loops and of `break`/`continue`

The checker SHALL require the condition of `for` and `while` to be `Boolean`, without truthiness, and SHALL reject `break`/`continue` outside a loop.

#### Scenario: Non-Boolean condition
- **WHEN** `while 1 { }` is written
- **THEN** a diagnostic indicating that the condition must be `Boolean` is emitted

#### Scenario: `break` outside a loop
- **WHEN** `break` appears outside any loop, even inside a nested function
- **THEN** a diagnostic pointing to the `break` is emitted

### Requirement: `for ... in` over the minimal iteration protocol

The checker SHALL accept `for ... in` over ranges (`0..N`, `0..=N`) and over
`String`, which iterates by graphemes binding an element of type `Char`. Over
any other type, it SHALL reject it, indicating that iteration of user-defined
types arrives with the Phase 3 traits.

While `Char` is not implemented, iteration of `String` SHALL be deferred
with the phase diagnostic, without binding an element of another type.

#### Scenario: Iteration over a range
- **WHEN** `for i in 0..10 { }` is written
- **THEN** `i` has type `Int32` within the body

#### Scenario: Iteration over an unsupported type
- **WHEN** `for x in value { }` is written and `value` is neither a range nor `String`
- **THEN** a diagnostic indicating that this type is not iterable yet is emitted

#### Scenario: Iteration over `String`
- **WHEN** `for c in text { }` is written with `text: String`
- **THEN** the bound element is a grapheme of type `Char`
- **AND** while `Char` is not implemented, the phase diagnostic is emitted instead of binding a `String`

### Requirement: `if` as an expression requires compatible branches

The checker SHALL accept `if`/`else` in expression position only when both branches are present and produce compatible types. In any other case, `if` is only valid as a statement.

#### Scenario: Compatible branches
- **WHEN** `if x > 0 { "positive" } else { "not positive" }` is evaluated in expression position
- **THEN** the resulting type is `String`

#### Scenario: `else` branch absent in expression position
- **WHEN** an `if` without `else` is used where a value is expected
- **THEN** a diagnostic indicating that the alternative branch is missing is emitted

#### Scenario: Branches of incompatible types
- **WHEN** the branches of an `if` used as an expression produce distinct, unrelated types
- **THEN** a diagnostic pointing to both types is emitted

### Requirement: Typing of optional, named, variadic parameters and default values

The checker SHALL verify that every call resolves to a valid assignment of arguments to parameters: named ones are matched by name, absent ones with a default value take it from the signature, and any left over are grouped into the variadic parameter if one exists.

#### Scenario: Optional parameter not provided
- **WHEN** `greet()` is called with `name?: String` and no argument
- **THEN** `name` has value `null` within the body

#### Scenario: Nonexistent named argument
- **WHEN** a call names an argument that does not exist in the signature
- **THEN** a diagnostic naming the unknown parameter is emitted

#### Scenario: Type of the variadic
- **WHEN** `sum(1, 2, 3)` is called with `...values: Int32`
- **THEN** `values` has the sequence type of `Int32` within the body

### Requirement: Typing of closures and immutable capture

The checker SHALL infer the type of a lambda from its parameters and its body, SHALL record which variables of the enclosing scope it captures, and SHALL reject mutation of a captured variable within the closure's body.

#### Scenario: Type of a lambda
- **WHEN** `inmut ADD = (a: Int32, b: Int32): Int32 => a + b;` is declared
- **THEN** `ADD` has function type from `(Int32, Int32)` to `Int32`

#### Scenario: Capture of an external variable
- **WHEN** a lambda references a variable declared in the scope that contains it
- **THEN** the check succeeds and the variable is recorded as captured

#### Scenario: Mutation of a captured variable
- **WHEN** the body of a lambda attempts to reassign a captured variable from the enclosing scope
- **THEN** a diagnostic indicating that the capture is immutable is emitted

### Requirement: Exhaustiveness of `match`

The checker SHALL require every `match` over an `enum` to cover all its constructors, or include the `_` wildcard. `match` over types without a closed set of values SHALL require the `_` wildcard as the final arm.

#### Scenario: `enum` completely covered
- **WHEN** a `match` over `Direction` has one arm for each of its four constructors
- **THEN** the check succeeds without requiring `_`

#### Scenario: Incomplete `enum` without a wildcard
- **WHEN** a `match` over `Direction` covers only two of its four constructors and has no `_`
- **THEN** a diagnostic naming the missing constructors is emitted

#### Scenario: `match` over `Int32` without a final wildcard
- **WHEN** a `match` over an `Int32` value does not end with a `_` arm
- **THEN** a diagnostic indicating that the wildcard is mandatory for that type is emitted

#### Scenario: Type of `match` as an expression
- **WHEN** all arms of a `match` used as an expression produce the same type
- **THEN** that is the type of the `match`

### Requirement: Null coalescing

The checker SHALL require both operands of `??` to share a common type, producing the non-nullable type when the right operand is not nullable.

#### Scenario: Coalescing with a non-nullable fallback
- **WHEN** `name ?? "anonymous"` is evaluated with `name: String?`
- **THEN** the result has type `String`

#### Scenario: Coalescing with a nullable fallback
- **WHEN** both operands of `??` are nullable
- **THEN** the result remains nullable

#### Scenario: `??` over a non-nullable left operand
- **WHEN** `??` is used over an expression of a non-nullable type
- **THEN** a diagnostic indicating that the operator is unnecessary is emitted

#### Scenario: Operands without a common type
- **WHEN** the operands of `??` do not share a common type
- **THEN** a diagnostic pointing to both types is emitted

### Requirement: Assignment between nullable and non-nullable types

The checker SHALL allow assigning a value of type `T` where `T?` is expected, and SHALL reject the opposite direction without explicit coalescing.

#### Scenario: Widen to nullable
- **WHEN** a `String` is assigned to a variable declared `String?`
- **THEN** the check succeeds

#### Scenario: Narrow without coalescing
- **WHEN** a `String?` is assigned to a variable declared `String`
- **THEN** a diagnostic is emitted
- **AND** the help suggests `??` to provide a default value

#### Scenario: `null` as a value
- **WHEN** `null` is assigned to a variable of a non-nullable type
- **THEN** a diagnostic indicating that the type does not allow absence of a value is emitted

### Requirement: `?.` is no longer deferred to the objects phase

The checker SHALL type `expr?.member` as the member's type in its nullable form when the receiver is nullable.

The deferral from the previous phase ends here: its reason was that no type had members.

#### Scenario: Safe access over a nullable value
- **WHEN** `user` has type `User?` and `name` is `String`
- **THEN** `user?.name` has type `String?`

#### Scenario: `?.` over a non-nullable value
- **WHEN** `?.` is used over an expression that is never null
- **THEN** a diagnostic indicating that the operator is unnecessary, with `.` as a suggestion, is emitted

#### Scenario: Nonexistent member
- **WHEN** `?.` names a member the type does not have
- **THEN** a diagnostic naming the member and the type is emitted

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
Records SHALL be immutable with structural field equality; value classes SHALL be distinct domain types without observable identity and SHALL be storable inline; an unmapped traditional enum case SHALL expose its exact case name as its default string value and no implicit numeric index; all arrays SHALL have fixed length; `String` SHALL be iterable; and a generator SHALL be both `Iterator<T>` and `Iterable<T>` while preserving locals between yields. Holding a `record` through a contract-typed reference SHALL NOT grant it observable identity or a mutation path back to the original value.

#### Scenario: Enum default and explicit mapping
- **WHEN** `Direction.North` has no mapping and `Code.North` maps to `"N"`
- **THEN** their observable mapped strings are `"North"` and `"N"` respectively, while both values retain their enum types

#### Scenario: Fixed inferred array
- **WHEN** an array literal contains three elements
- **THEN** its length is fixed at three and append/remove operations are rejected

#### Scenario: Structural field equality compares every field
- **WHEN** `==` compares two values of the same `record` or `value class` type
- **THEN** the result is the conjunction of each field's own equality, recursing into a nested `record`/`value class` field

#### Scenario: Structural field equality short-circuits on the first difference
- **WHEN** two `record`/`value class` values differ in their first field
- **THEN** `==` evaluates `false` without necessarily comparing the remaining fields

#### Scenario: A contract-typed view of a record grants no new identity
- **WHEN** the same `record` value is held through two separately-produced contract-typed references
- **THEN** `is` between them is not guaranteed `true`, and neither reference exposes a way to mutate the original value's storage

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

### Requirement: Public type taxonomy
The language SHALL distinguish compiler primitives, native reference types, user-defined value/reference types, and special types while retaining `Object` as their conceptual root. Storage inline or behind a reference SHALL NOT remove a value from the common type and contract system.

#### Scenario: Primitive with methods
- **WHEN** an `Int32` value invokes `abs()` or `to_string()`
- **THEN** the operation resolves through its native capabilities without boxing being observable

### Requirement: Float family replaces Decimal family
The binary floating family SHALL be `Float16`, `Float32`, `Float64`, and `Float128`, with `Float` aliasing `Float64` and ordinary fractional literals inferring `Float64`. `NaN` SHALL NOT be a valid Zirk value; indeterminate operations SHALL produce controlled errors.

`Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec` and `Decimal` SHALL NOT be recognized as types of the language, nor announced as types of a future phase. An exact base-ten type may later arrive as a standard-library type, and it would be a different thing from `Float`.

#### Scenario: Default fractional literal
- **WHEN** `1.5` has no contextual type
- **THEN** its inferred type is `Float64`

#### Scenario: Indeterminate infinity operation
- **WHEN** positive infinity is subtracted from positive infinity
- **THEN** a controlled arithmetic error is produced instead of `NaN`

#### Scenario: Withdrawn Decimal family
- **WHEN** an annotation names `Decimal64`
- **THEN** an unknown-type diagnostic is emitted
- **AND** no arrival phase is announced for that name

### Requirement: Deep contextual conversion
An explicit numeric or String constructor around an operator expression SHALL establish the target domain for the contained compatible arithmetic or concatenation tree, converting operands before those operators execute. The context SHALL NOT mutate operands or propagate through a called function's body.

#### Scenario: Contextual floating division
- **WHEN** `a` and `b` are integers equal to 3 and 4 and `Float(a / b)` is evaluated
- **THEN** division occurs in the Float domain and returns `0.75`

#### Scenario: Contextual String concatenation
- **WHEN** `String("value=" + 42)` is evaluated
- **THEN** the integer operand is converted before concatenation and the result is `"value=42"`

### Requirement: Reference mutability and strict aliases
For reference types, `mut` SHALL permit binding reassignment and referent mutation, `inmut` SHALL prohibit reassignment but permit referent mutation, and `inmut::strict` SHALL prohibit both. A strict reference SHALL NOT yield a mutable alias or be acquired while an accessible mutable alias exists; an independent `clone()` MAY be mutable.

#### Scenario: Inmut String element update
- **WHEN** an `inmut String` binding assigns a valid `Char` to one element
- **THEN** the shared referenced String is updated while binding reassignment remains prohibited

#### Scenario: Strict-to-mutable alias
- **WHEN** code assigns an `inmut::strict String` reference to a `mut` binding without cloning
- **THEN** type checking rejects the alias

### Requirement: Grapheme Char
`Char` SHALL represent exactly one Unicode grapheme, potentially containing multiple code points and bytes. `ascii_code()` SHALL return its ASCII code only when the grapheme is exactly one ASCII scalar and `-1` otherwise; case transformations SHALL return `String`.

#### Scenario: Multi-code-point grapheme
- **WHEN** a family emoji literal contains one extended grapheme
- **THEN** it is a valid single `Char` even though it contains multiple code points

#### Scenario: Non-ASCII code
- **WHEN** `ascii_code()` is invoked on `'π'`
- **THEN** it returns `-1`

### Requirement: Native String reference semantics and operators
`String` SHALL be a native reference type with shared mutation, explicit deep cloning, grapheme indexing/slicing, content equality, identity testing, checked concatenation, and checked repetition by a non-negative integer in either operand order.

#### Scenario: Shared String mutation
- **WHEN** two mutable bindings alias one String and one assigns a grapheme at an index
- **THEN** both bindings observe the changed content

#### Scenario: String repetition
- **WHEN** `"ja" * 3` or `3 * "ja"` is evaluated
- **THEN** the result is `"jajaja"`

#### Scenario: Negative repetition
- **WHEN** a String repetition count is negative
- **THEN** a controlled invalid-count error is produced

### Requirement: Native operator contracts by type
Each native type SHALL expose only its documented operator set. Integer division SHALL truncate toward zero, remainder SHALL preserve the dividend sign, mixed integer/Float arithmetic SHALL produce Float, Boolean SHALL have no truthiness, and unsupported operations SHALL fail at type checking.

#### Scenario: Signed remainder
- **WHEN** `-10 % 3` is evaluated
- **THEN** the result is `-1`

#### Scenario: Boolean arithmetic
- **WHEN** application code attempts `true + false`
- **THEN** type checking rejects the operation

### Requirement: `Float` family and temporal types recognized as pending

The checker SHALL recognize `Float16`, `Float32`, `Float64`, `Float128`,
`Float`, the unimplemented integer widths, `Char`, and the temporal types
`Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`,
and `Period` as pending language types, each declaring the phase that
brings it.

#### Scenario: Annotation with a Float type
- **WHEN** `mut ratio: Float64 = 0;` is declared
- **THEN** the diagnostic names the type and indicates the phase in which it arrives
- **AND** it is NOT reported as a nonexistent type

#### Scenario: Annotation with a temporal type
- **WHEN** an annotation names `Instant` or `Duration`
- **THEN** the diagnostic indicates the phase of the temporal family

### Requirement: Observable identity and equality of `String`

The checker and runtime SHALL treat `String` as a reference with observable
identity: `is` SHALL compare referent identity and `==` SHALL compare
content.

Content comparison SHALL be insensitive to the Unicode normalization form
of the operands. The hash of a `String` SHALL derive from its canonical
form, so that two strings equal by `==` never produce distinct hashes.

#### Scenario: Equality insensitive to normalization
- **WHEN** two strings with the same perceived content, one in NFC and the other in NFD, are compared with `==`
- **THEN** the result is `true`

#### Scenario: Identity versus content
- **WHEN** two distinct bindings alias the same `String` and a third has the same content in another referent
- **THEN** `is` is `true` only for the first two and `==` is `true` for all three

#### Scenario: Consistency between hash and equality
- **WHEN** two strings equal by `==` are used as keys in a map
- **THEN** they resolve to the same entry

### Requirement: Projection copy and whole-reference aliasing
The checker SHALL classify reference expressions as whole references, projection reads, or places. Whole-reference assignment/passing/return/capture SHALL preserve aliasing; projection reads SHALL require deep Clone and produce independence; places SHALL preserve access to original storage. Destructuring, matching, callable capture, collection extraction, and generic T SHALL follow the same rule.

#### Scenario: Generic projection needs Clone
- **WHEN** a generic function returns `values[0]` for unconstrained T
- **THEN** the checker requires `T from Clone` or rejects extraction

### Requirement: Multiple bindings and simultaneous assignment are atomic at the language level
Comma-grouped declarations SHALL apply their declared type and binding
permission to every name and SHALL require initializer arity to match when an
initializer list is present. Missing initializers SHALL use the declared type's
default independently for every binding. Simultaneous assignment SHALL require
equal source and destination arity, evaluate every source exactly once from
left to right before any destination write, type-check values positionally,
reject duplicate destinations, and then commit writes from left to right.
Rebinding `inmut` or `inmut::strict`, or mutating a projection through an
`inmut::strict` referent, SHALL be rejected.

#### Scenario: Swap observes original values
- **WHEN** `left` is `3`, `right` is `4`, and source executes
  `left, right = right, left`
- **THEN** `left` becomes `4` and `right` becomes `3` without either source
  observing an earlier destination write

#### Scenario: Strict reference projection is a destination
- **WHEN** a simultaneous assignment attempts to write an element through an
  `inmut::strict` collection reference
- **THEN** compilation rejects the write before evaluating an executable update

### Requirement: Final callable and object typing supersedes delivery limits
The final language type system SHALL support structural `Fn` adaptation, escaping closures, compiler-managed capture environments, abstract-class implementation, explicit overrides, declared generic variance, normalized unions, constant tuple indexes, copied iteration, and exhaustive guard-free matching. Feature phasing MAY diagnose an undelivered construct but SHALL NOT describe the final construct as semantically forbidden.

#### Scenario: Phase-limited closure
- **WHEN** the current compiler phase does not yet implement escaping closures
- **THEN** its diagnostic identifies the delivery phase while documentation retains the final legal Fn semantics

### Requirement: Default initialization and immutable data
Every omitted attribute SHALL receive its type default. Construction MAY finalize an `inmut` attribute before the object becomes available. Records and tuples SHALL remain immutable values; enums SHALL remain closed data without user methods; collections SHALL enforce referent permissions and strict aliases.

#### Scenario: Omitted class attribute
- **WHEN** an Int32 class attribute has no initializer and construct does not replace it
- **THEN** its value is zero after construction

### Requirement: Failure effects and Result consumption are checked
The checker SHALL require explicit exceptions to be handled or declared, SHALL propagate declared exception sets through calls and callable compatibility, SHALL permit documented implicit `RuntimeError` exceptions without signature declaration, and SHALL reject an unconsumed `Result` except through explicit discard.

#### Scenario: Callable throws too broadly
- **WHEN** a callable declaring `throws StorageError` is assigned to `Fn() => Void`
- **THEN** assignment fails because the target does not permit that explicit exception

### Requirement: Resource responsibility is flow-sensitive
The checker SHALL track managed, transferred, closed, dependent, and abandoned resource responsibility through branches, returns, containers, closures, tasks, and exceptional exits. It SHALL reject statically provable duplicate close, use-after-transfer, illegal escape, non-cloneable projection, and leak paths.

#### Scenario: Every branch transfers or closes
- **WHEN** all control-flow paths either close or transfer one resource responsibility
- **THEN** the function satisfies resource lifetime checking

### Requirement: Permission effects propagate outside surface callable syntax
The checker SHALL retain compiler-internal permission-effect metadata on declarations and callable values, infer it transitively through higher-order calls, and report a path from entry point to privileged API. Permission effects SHALL NOT alter the written `Fn(P...) => R` grammar.

#### Scenario: Higher-order permission propagation
- **WHEN** a permission-free wrapper invokes a callback whose concrete value reads a secret
- **THEN** the call site and application acquire the secret-read requirement in compiler metadata

### Requirement: Memory and task type family
The type system SHALL define `Weak<T>`, `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>`, `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, synchronization types, and their capability constraints without exposing mandatory ownership or lifetime parameters.

#### Scenario: Await type is inferred
- **WHEN** an expression has type `Task<Result<User, LoadError>>`
- **THEN** awaiting it has type `Result<User, LoadError>`

#### Scenario: NativeSlice element type is ABI-safe only
- **WHEN** `NativeSlice<T>`/`NativeSliceMut<T>` is instantiated with an element type outside the ABI-safe subset (`Void`/`Boolean`/fixed-width `Int`/`UInt`/`Float32`/`Float64`/nested `Pointer<T>`)
- **THEN** the checker rejects the instantiation, naming the unsupported element type

### Requirement: Derived concurrent capabilities
Transferability and shareability SHALL be compiler-derived, non-forgeable properties based on the complete reachable type graph, mutability, resource ownership, and synchronization contract.

#### Scenario: Class contains mutex
- **WHEN** a class safely encapsulates mutable state behind a supported mutex
- **THEN** the compiler may derive sharing without exposing an ordinary user-implemented marker

### Requirement: Nominal types and subtyping

The checker SHALL treat every class, record, value class and enum as a distinct nominal type, and SHALL accept a value where one of its superclasses or an implemented contract is expected.

#### Scenario: Subclass where base is expected
- **WHEN** an instance of `Admin` is passed to a parameter of type `User`
- **THEN** the check succeeds

#### Scenario: Implementation where contract is expected
- **WHEN** an instance is passed to a parameter whose type is a contract it implements
- **THEN** the check succeeds

#### Scenario: Two types with the same shape are not the same type
- **WHEN** two classes declare the same fields and one is assigned where the other is expected
- **THEN** a diagnostic is emitted: equivalence is by name, not by shape

#### Scenario: Base where subclass is expected
- **WHEN** an instance of `User` is passed to a parameter of type `Admin`
- **THEN** a diagnostic is emitted

### Requirement: Member resolution

The checker SHALL resolve an `expr.member` access against the type of `expr` and its inheritance chain, respecting visibility.

#### Scenario: Inherited member
- **WHEN** a field declared in the superclass is accessed
- **THEN** it resolves to that declaration

#### Scenario: Nonexistent member
- **WHEN** a member that no ancestor declares is accessed
- **THEN** a diagnostic naming the member and the type is emitted

#### Scenario: Member hidden by visibility
- **WHEN** a `private` member is accessed from outside
- **THEN** a visibility diagnostic is emitted, distinct from the nonexistent-member diagnostic

### Requirement: Shadowing and qualified capture

The checker SHALL reject a local declaration that hides another still-visible local or parameter. A lambda parameter MAY share a capture's name only when the capture is referenced as `this.name`; the plain name denotes the parameter.

#### Scenario: Duplicate local in nested scope
- **WHEN** an inner block declares a local name that is still visible
- **THEN** a diagnostic pointing to both declarations is emitted

#### Scenario: Parameter and capture collision
- **WHEN** a lambda declares parameter `prefix` and reads `this.prefix`
- **THEN** `prefix` resolves to the parameter and `this.prefix` to the outer capture

### Requirement: Object reference strictness

For class references, `mut` SHALL permit rebinding and mutating the object,
`inmut` SHALL prevent only rebinding, and `inmut::strict` SHALL prevent
reachable mutation. A strict reference SHALL NOT become a mutable alias or be
acquired while a mutable alias remains accessible.

#### Scenario: Mutable clone from a strict reference
- **WHEN** a strict object implements `Clone` and is cloned to a `mut` binding
- **THEN** the independent clone can be mutated without altering the original object

### Requirement: Constructor resolution

The checker SHALL select among multiple `construct` declarations by arity, types, optionals and names, SHALL allow reordering named arguments, and SHALL reject duplicate effective signatures or ambiguous calls.

#### Scenario: Reordered named construction
- **WHEN** `User(name: "Cristian", id: 1)` matches `construct(id: UInt64, name: String)`
- **THEN** that signature is selected and each value binds by name

### Requirement: Checked casts

A cast to a related type SHALL be checked at runtime and fail in a controlled manner; a cast between unrelated types SHALL be rejected at compile time.

#### Scenario: Valid downcast
- **WHEN** a `User` that is actually an `Admin` is cast to `Admin`
- **THEN** the result is the instance

#### Scenario: Invalid downcast
- **WHEN** the value is not of the requested type
- **THEN** the program terminates with a diagnosed runtime error
- **AND** it does NOT incur undefined behavior

#### Scenario: Unrelated types
- **WHEN** a cast occurs between two types that share neither hierarchy nor contract
- **THEN** a diagnostic is emitted at compile time

### Requirement: Operators resolve by contract

The checker SHALL resolve an operator by looking up the contract implemented by its type, instead of comparing against a fixed list of types.

#### Scenario: Concatenation
- **WHEN** `"a" + "b"` is evaluated
- **THEN** the resulting type is `String`

#### Scenario: Operand without the contract
- **WHEN** an operand does not implement the operator's contract
- **THEN** the diagnostic names the missing contract, not a list of allowed types

### Requirement: The final function type syntax is not delivered in Phase 3

The Phase 3 checker SHALL reject any position that requires annotating the
type of a closure —parameter, return or attribute— with a diagnostic indicating
that `Function(P...) => R` / `Fn(P...) => R` arrives in a later phase.

This is an implementation limit, not the final semantics. The language already
defines compatibility by signature, escapable closures and automatic storage
per `docs/CORE_LANGUAGE_SEMANTICS.md`.

#### Scenario: Closure in a local variable
- **WHEN** `inmut F = (a: Int32): Int32 => a + 1;` is declared and `F(1)` is called
- **THEN** the check succeeds
- **AND** the type of `F` is inferred without being written

#### Scenario: Closure as parameter type
- **WHEN** a function declares a parameter whose type attempts to be a function
- **THEN** a diagnostic is emitted indicating that function types arrive in a later phase
- **AND** it is NOT reported as an unknown type

#### Scenario: Closure as return type
- **WHEN** a function declares that it returns a closure
- **THEN** the same diagnostic is emitted

#### Scenario: Closure as attribute type
- **WHEN** a class declares an attribute whose type attempts to be a function
- **THEN** the same diagnostic is emitted

#### Scenario: Returning a locally created closure
- **WHEN** a function builds a closure and returns it
- **THEN** a diagnostic is emitted indicating that a closure cannot escape the function that creates it

### Requirement: `to_string()` contract

The language SHALL define `to_string()` as a reserved contract, in the same
family as the operator contracts (`ZIRK_LANGUAGE_SPEC.md` section 4): every
scalar native type implements it, and a user type —class, record, enum— may
implement it to produce its own textual representation. `print`/`println` SHALL
route through it instead of accepting only a closed list of types.

#### Scenario: Native type printable without declaring anything
- **WHEN** an `Int32`, `Float64`, `Boolean` or `Char` is printed
- **THEN** the check succeeds without the program declaring `to_string()` for them

#### Scenario: User type that implements `to_string()`
- **WHEN** a class declares `to_string(): String` and an instance is passed to `println`
- **THEN** the check succeeds and the printed value is the one produced by that method

#### Scenario: User type that does not implement it
- **WHEN** a class without `to_string()` is passed to `println`
- **THEN** a diagnostic naming the type is emitted, distinct from a generic `TYPE_MISMATCH`

### Requirement: String interpolation

A `String` literal SHALL allow `{expr}` inside it, desugared to concatenating
the literal text with `expr.to_string()` for each interpolated expression, in
the order they appear.

#### Scenario: Interpolation of a variable
- **WHEN** `"User: {user.name}"` is written
- **THEN** the result concatenates the literal text with `user.name.to_string()`

#### Scenario: Interpolation of a non-printable type
- **WHEN** the interpolated expression has a type without `to_string()`
- **THEN** the same diagnostic is emitted as passing that value directly to `println`

### Requirement: Fractional literal context and mixed arithmetic

An unannotated fractional literal SHALL have type `Float64`, and an arithmetic
operation between an integer type and a `Float` type SHALL produce `Float`.
Section 3 of `ZIRK_LANGUAGE_SPEC.md` already documented both rules; this phase
is the first in which `Float` exists and they become verifiable.

#### Scenario: Unannotated fractional literal
- **WHEN** `mut x = 1.5;` is written without a type annotation
- **THEN** `x` has type `Float64`

#### Scenario: Mixed integer and `Float` arithmetic
- **WHEN** an `Int32` and a `Float64` are added
- **THEN** the result has type `Float64`

### Requirement: `Never` is the bottom type

The checker SHALL treat `Never` as assignable to any type, and as contributing nothing to the shared type at a branch join (`??`, `match`, `if`/ternary as an expression): the join's type SHALL be the other branch's type, never a nullable widening of it. No expression SHALL ever produce a runtime value of type `Never`; the only construct typed `Never` is a call to the compiler-known `fatalError(message: String): Never`, which never returns.

#### Scenario: `Never` is assignable anywhere
- **WHEN** `fatalError("unreachable")` is used where `Int32`, `String`, or `Boolean` is expected
- **THEN** the checker accepts it

#### Scenario: `Never` contributes nothing at a ternary join
- **WHEN** `cond ? 5 : fatalError("unreachable")` is checked
- **THEN** its type is `Int32`, not `Int32?` and not `Never`

#### Scenario: An enum with no variants is rejected
- **WHEN** `enum Impossible { }` is declared
- **THEN** the checker rejects it and names `Never` as the type that already means "no value"

### Requirement: Enum declaration order is independent of self- and mutual references

Declaration order between an enum and a class, or between two enums, that reference each other SHALL NOT matter. An enum's variant payload naming its own type, or another enum's, directly and with no indirection in between SHALL be rejected at compile time rather than compiled into an unbounded-size representation.

#### Scenario: Enum and class reference each other regardless of order
- **WHEN** an enum's variant payload names a class, and that class has a field naming the enum back, in either declaration order
- **THEN** both declarations resolve correctly

#### Scenario: A directly self-referential enum field is rejected, not miscompiled
- **WHEN** an enum's variant payload names its own type with no indirection in between (for example `enum IntList { Nil, Cons(head: Int32, tail: IntList) }`)
- **THEN** compilation fails with a diagnostic naming the cycle, rather than crashing or producing an unbounded-size type

### Requirement: Generic substitution recurses into a nested instantiation

The checker SHALL replace a type parameter appearing inside a nested generic instantiation (an enum or contract instantiation named as part of a parameter's declared type), not only when the parameter's own declared type is directly the type parameter itself, and SHALL infer a type parameter's solution from an argument's type the same way when the declared parameter type nests it.

#### Scenario: Inference through a nested generic parameter type
- **WHEN** a generic function declares a parameter of type `Option2<T>` and is called with an argument of type `Option2<Int32>`
- **THEN** `T` is inferred as `Int32`

#### Scenario: Substitution replaces a type parameter inside a nested instantiation
- **WHEN** a generic enum's own variant payload names another generic instantiation containing the enum's type parameter (for example `Bar<Baz<T>>`)
- **THEN** substituting `T` with a concrete type produces the fully-substituted nested instantiation, not a partially-substituted one

