## ADDED Requirements

### Requirement: Public type taxonomy
The language SHALL distinguish compiler primitives, native reference types, user-defined value/reference types, and special types while retaining `Object` as their conceptual root. Storage inline or behind a reference SHALL NOT remove a value from the common type and contract system.

#### Scenario: Primitive with methods
- **WHEN** an `Int32` value invokes `abs()` or `to_string()`
- **THEN** the operation resolves through its native capabilities without boxing being observable

### Requirement: Float family replaces Decimal family
The binary floating family SHALL be `Float16`, `Float32`, `Float64`, and `Float128`, with `Float` aliasing `Float64` and ordinary fractional literals inferring `Float64`. `NaN` SHALL NOT be a valid Zirk value; indeterminate operations SHALL produce controlled errors.

#### Scenario: Default fractional literal
- **WHEN** `1.5` has no contextual type
- **THEN** its inferred type is `Float64`

#### Scenario: Indeterminate infinity operation
- **WHEN** positive infinity is subtracted from positive infinity
- **THEN** a controlled arithmetic error is produced instead of `NaN`

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
