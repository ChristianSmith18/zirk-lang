# zirk-type-system Delta

## MODIFIED Requirements

### Requirement: `to_string()` contract

The language SHALL define `to_string()` as a reserved contract, in the same
family as the operator contracts (`ZIRK_LANGUAGE_SPEC.md` section 4): every
type that can produce a value answers it — native scalars, temporals,
collections, tuples, records, classes, enums, callables, weak references,
opaque handles (`Job`, `Channel`, `Pointer`, `NativeSlice`, `Dependent`,
`Pin`), `Regex`, `Range`, `Map`, `Set` and nullable types — with a
compiler-provided default when the type does not declare its own. A user
type —class, record, enum— may implement it to produce its own textual
representation. Only `Void` and `Never` have no text form.
`print`/`println` SHALL route through it instead of accepting a closed list
of types. The renderings themselves are owned by the
`universal-string-rendering` capability.

#### Scenario: Native type printable without declaring anything

- **WHEN** an `Int32`, `Float64`, `Boolean` or `Char` is printed
- **THEN** the check succeeds without the program declaring `to_string()` for them

#### Scenario: Previously unprintable types are printable

- **WHEN** a `List<Int32>`, `Map<String, Int32>`, `Range<Int32>`,
  `Regex`, `Job<Int32>` or `Pointer<Int32>` value is passed to `println` or
  `to_string()`
- **THEN** the check succeeds and the value renders per
  `universal-string-rendering`

#### Scenario: Nullable receiver

- **WHEN** `x: Int32?` is passed to `x.to_string()` or `println(x)`
- **THEN** the check succeeds; the result is `<Null>` when absent and the
  inner rendering when present

#### Scenario: User type that implements `to_string()`

- **WHEN** a class declares `to_string(): String` and an instance is passed to `println`
- **THEN** the check succeeds and the printed value is the one produced by that method

#### Scenario: Non-value types rejected

- **WHEN** an expression of type `Void` or `Never` is passed to `println`,
  `to_string()` or interpolation
- **THEN** a diagnostic naming the type is emitted, distinct from a generic
  `TYPE_MISMATCH`

### Requirement: String interpolation

A `String` literal — whether `"…"` or `'''…'''` — SHALL allow `{expr}`
inside it, desugared to concatenating the literal text with
`expr.to_string()` for each interpolated expression, in the order they
appear. Any value whose type is not `Void`/`Never` is legal.

#### Scenario: Interpolation of a variable

- **WHEN** `"User: {user.name}"` is written
- **THEN** the result concatenates the literal text with `user.name.to_string()`

#### Scenario: Interpolation of a collection or nullable

- **WHEN** `"items: {list}"` or `"maybe: {nullable}"` is written with a
  `List` or `T?` expression
- **THEN** the result concatenates the literal text with the value's
  `to_string()` rendering, including `<Null>` for an absent value

#### Scenario: Interpolation of a non-value type

- **WHEN** the interpolated expression has type `Void` or `Never`
- **THEN** the same diagnostic is emitted as passing that value directly to
  `println`
