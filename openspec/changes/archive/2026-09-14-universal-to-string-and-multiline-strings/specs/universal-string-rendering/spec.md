# universal-string-rendering Specification

## Purpose

Define the complete `to_string()` contract: what every Zirk type renders as
text, which types are excluded, and how `print`/`println`/interpolation route
through it. This capability is the single owner of the render table; per-type
specs describe member surfaces, not rendering semantics.

## ADDED Requirements

### Requirement: Universal `to_string()` contract

Every type that can produce a value SHALL answer `to_string(): String`,
either through a compiler-provided default rendering or a user-declared
method. `Void` and `Never` are the only types without a text form — they
produce no value.

`print`, `println` and `{expr}` string interpolation SHALL each accept any
value whose type is not `Void`/`Never` and convert it through `to_string()`.
No closed printable-type list SHALL gate these operations.

#### Scenario: Scalar prints without declaration

- **WHEN** an `Int32`, `Float64`, `Boolean`, `Char` or `String` value is
  passed to `println`
- **THEN** the program compiles and prints its text form

#### Scenario: Collection prints without declaration

- **WHEN** a `List<Int32>` holding `[1, 2, 3]` is passed to `println` or
  interpolated in `"{list}"`
- **THEN** the program compiles and prints `[1, 2, 3]`

#### Scenario: Non-value types rejected

- **WHEN** an expression of type `Void` or `Never` is passed to `println`,
  `print`, `to_string()` or interpolation
- **THEN** a diagnostic is emitted stating that the type has no text form

### Requirement: Scalar renderings

Integer types (`Int8`…`Int128`, `UInt8`…`UInt128`) SHALL render in base ten
with a leading `-` for negatives. `IntN.to_string(radix: n)` SHALL render in
base `n` (clamped to 2–36), sign-magnitude form, lowercase digits.

`Float`/`Decimal` SHALL render as an exact base-ten decimal. `Float16` SHALL
render through `Float32` widening (exact); `Float32`/`Float64` SHALL render
the shortest round-trip form (`1.0` renders `1`); `Float128` SHALL render
through `Float64` truncation (documented lossy). `Boolean` SHALL render
`true`/`false`. `Char` SHALL render as the grapheme itself. `String` SHALL
render as itself.

#### Scenario: Integer radix

- **WHEN** `255.to_string(radix: 16)` and `(-255).to_string(radix: 2)` run
- **THEN** the results are `ff` and `-11111111`

#### Scenario: Binary float without fractional part

- **WHEN** `1.0f64.to_string()` runs
- **THEN** the result is `1`

#### Scenario: Exact decimal

- **WHEN** `0.001.to_string()` runs
- **THEN** the result is `0.001`

### Requirement: Temporal renderings

`Duration` SHALL render as a normalized literal in the largest unit the
magnitude reaches, with an exact decimal fraction for the remainder and a
leading `-` for negatives (`1500ms` → `1.5s`, `250ms` → `250ms`, `90m` →
`1.5h`, `0` → `0s`, `-5s` → `-5s`). `Date` SHALL render ISO 8601
`YYYY-MM-DD`. `Time` SHALL render `HH:MM:SS` with an optional `.fffffffff`
fraction trimmed of trailing zeros. `DateTime` SHALL render
`YYYY-MM-DDTHH:MM:SS[.fffffffff]`.

#### Scenario: Duration picks largest unit

- **WHEN** `1500ms.to_string()` and `90m.to_string()` run
- **THEN** the results are `1.5s` and `1.5h`

#### Scenario: Time with sub-second fraction

- **WHEN** `Time(14, 30, 15, 500000000).to_string()` runs
- **THEN** the result is `14:30:15.5`

### Requirement: Sequence rendering

`Array<T>` and `List<T>` SHALL render as `[e0, e1, …]` with elements
converted recursively through `to_string()`, `, ` separators, and `[]` for
empty. Element strings are not quoted.

#### Scenario: Nested lists

- **WHEN** `[[1, 2], [3]].to_string()` runs on a `List<List<Int32>>`
- **THEN** the result is `[[1, 2], [3]]`

#### Scenario: String elements unquoted

- **WHEN** `["a", "b"].to_string()` runs on a `List<String>`
- **THEN** the result is `[a, b]`

### Requirement: Deterministic `Map`/`Set` rendering

`Map<K, V>` SHALL render as `{k0: v0, k1: v1, …}` and `Set<T>` as
`{e0, e1, …}`, with entries ordered deterministically: the natural order of
the key/element type when one exists (numeric for `IntN`/`UIntN`/`Decimal`/
`FloatN`, lexicographic for `String`, chronological for `Date`/`Time`/
`DateTime`/`Duration`), and rendered-text order otherwise. Empty collections
render `{}`.

#### Scenario: Map sorts by key

- **WHEN** a `Map<Int32, String>` containing `{2: "b", 10: "a", 1: "c"}` is
  converted with `to_string()`
- **THEN** the result is `{1: c, 2: b, 10: a}` — numeric order, not
  lexicographic

#### Scenario: Set output is stable across runs

- **WHEN** the same `Set<String>` is rendered in two separate executions
- **THEN** both renderings are identical

### Requirement: `Range` rendering mirrors its literal

`Range<T>` SHALL render the source-literal shape: `start..end` for an
exclusive bound, `start..=end` for an inclusive bound, and `:step` appended
when a step is present (`1..10`, `1..=10`, `1..10:2`, `1..=10:2`).

#### Scenario: Stepped inclusive range

- **WHEN** `(1..=10:2).to_string()` runs
- **THEN** the result is `1..=10:2`

### Requirement: Nullable rendering

A `T?` value SHALL answer `to_string()` directly (no narrowing required): a
present value renders through the inner type's `to_string()`, and an absent
value renders `<Null>`. The `null` literal SHALL render `<Null>`.
`x?.to_string()` SHALL keep safe-navigation semantics and return `String?`.

#### Scenario: Absent value

- **WHEN** `x: Int32? = null` and `x.to_string()` runs
- **THEN** the result is `<Null>`

#### Scenario: Present value

- **WHEN** `x: Int32? = 5` and `x.to_string()` runs
- **THEN** the result is `5`

#### Scenario: Null literal in a list

- **WHEN** a `List<Int32?>` holding `[1, null, 3]` is rendered
- **THEN** the result is `[1, <Null>, 3]`

### Requirement: Structured-type rendering uses surface names

`record`, `class`, and tuple values without a declared `to_string()` SHALL
render `Name(field: v, …)` — or `(v0, v1, …)` for tuples — using the
**surface** type name: the `$local$N` checker mangling of function-local
classes SHALL be stripped, and `Outer.Inner` qualification SHALL be kept for
nested classes. Compiler-synthesized fields that are not user-declared (such
as an `inner class`'s `outer` capture) SHALL NOT appear in the rendering. A
class or record declaring `to_string(): String` SHALL use its method.

#### Scenario: Local class shows surface name

- **WHEN** a class `Ejemplo` declared inside a function is rendered
- **THEN** the output starts with `Ejemplo(` — never `Ejemplo$local$`

#### Scenario: Nested class shows qualified name

- **WHEN** an `inner class Ejemplo3` inside `Ejemplo2` is rendered
- **THEN** the output starts with `Ejemplo2.Ejemplo3(` and contains no
  `outer` field

#### Scenario: Declared method wins

- **WHEN** a class declaring `to_string()` returning `"custom"` is rendered
- **THEN** the result is `custom`

### Requirement: Enum rendering includes payloads

An enum value SHALL render `Variant` for field-less cases and
`Variant(field: v, …)` for payload cases, using the declared payload field
names and rendering each value recursively. `Result<T, E>` follows the same
rule: `Ok(5)`, `Error("boom")`. The static `E.to_string`/`E.to_string()`
member SHALL keep rendering the enum's own name.

#### Scenario: Payload case

- **WHEN** `Forma.Circulo(radio: 5).to_string()` runs
- **THEN** the result is `Circulo(radio: 5)`

#### Scenario: Traditional case

- **WHEN** `Color.Rojo.to_string()` runs
- **THEN** the result is `Rojo`

#### Scenario: Result rendering

- **WHEN** `Ok(5)` and `Error("boom")` values of `Result<Int32, String>`
  are rendered
- **THEN** the results are `Ok(5)` and `Error(boom)`

### Requirement: Callable rendering

A value of function type SHALL render `<Function name>` when it refers to a
named function and `<Function>` when it is an anonymous lambda or closure
without a name.

#### Scenario: Named function

- **WHEN** `main` is passed to `println` without call parentheses
- **THEN** the output is `<Function main>`

#### Scenario: Anonymous lambda

- **WHEN** a lambda bound to a variable is rendered
- **THEN** the output is `<Function>`

### Requirement: Opaque handle rendering

Opaque handle types SHALL render a constant `<TypeName>`: `Job<T>` →
`<Job>`, `Channel<T>` → `<Channel>`, `Pointer<T>` → `<Pointer>`,
`NativeSlice<T>` → `<NativeSlice>`, `NativeSliceMut<T>` →
`<NativeSliceMut>`, `Dependent<T>` → `<Dependent>` and `Pin<T>` → `<Pin>` —
the same convention `Weak<T>`'s `<Weak>` established.

#### Scenario: Job handle

- **WHEN** a `Job<Int32>` value is rendered
- **THEN** the result is `<Job>`

### Requirement: `Regex` rendering

`Regex` SHALL answer `to_string()` with its source pattern wrapped in the
literal spelling: `re'pattern'`.

#### Scenario: Pattern literal

- **WHEN** `re'a+'.to_string()` runs
- **THEN** the result is `re'a+'`

### Requirement: Contract dispatch

Contract receivers SHALL dispatch `to_string()` to the concrete
implementer's method, exactly like any other contract call — a value
reached through a contract that declares `to_string(): String` renders per
that method.

#### Scenario: Interface receiver

- **WHEN** a value typed as an interface declaring `to_string()` is rendered
- **THEN** the implementing class's method runs
