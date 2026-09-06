# zirk-data-types delta

## ADDED Requirements

### Requirement: Tuple members

A tuple value SHALL expose `length` (compile-time element count, `Int32`)
and `to_string()` (readable rendering) in addition to constant indexing
and destructuring.

#### Scenario: Tuple length

- **WHEN** `(1, "x", true).length` is read
- **THEN** the result is `3`

#### Scenario: Tuple rendering

- **WHEN** `(1, "x").to_string()` runs
- **THEN** the result is a `String` naming both elements

### Requirement: Traditional enum case members

A traditional-enum case SHALL expose `name` (declared case name,
`String`), `value` (explicit mapping or the case name by default), and
`to_string()` (the case name).

#### Scenario: Case name

- **WHEN** `Color.Red.name` is read on `enum Color { Red, Green }`
- **THEN** the result is `"Red"`

#### Scenario: Mapped value

- **WHEN** `Color.Red.value` is read on a `String`-mapped enum
- **THEN** the result is the mapped value

### Requirement: Universal rendering on structured values

`record`, algebraic-enum, `class`, `Weak<T>`, and callable values SHALL
answer `to_string()` with a readable default rendering when the type does
not declare its own.

#### Scenario: Record default rendering

- **WHEN** `Point(x: 1, y: 2).to_string()` runs on a record without a
  declared `to_string`
- **THEN** the result is a `String` naming the type
