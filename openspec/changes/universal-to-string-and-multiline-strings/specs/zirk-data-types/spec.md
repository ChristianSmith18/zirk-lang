# zirk-data-types Delta

## MODIFIED Requirements

### Requirement: Universal rendering on structured values

`record`, algebraic-enum, `class`, `Weak<T>`, and callable values SHALL
answer `to_string()` with a readable default rendering when the type does
not declare its own. The default SHALL use the surface type name (the
`$local$N` mangling of local classes is stripped, `Outer.Inner`
qualification is kept), SHALL include algebraic-enum payloads as
`Variant(field: v, …)`, and SHALL NOT expose compiler-synthesized fields
such as an `inner class`'s `outer` capture. The complete render table is
owned by the `universal-string-rendering` capability.

#### Scenario: Record default rendering

- **WHEN** `Point(x: 1, y: 2).to_string()` runs on a record without a
  declared `to_string`
- **THEN** the result is `Point(x: 1, y: 2)`

#### Scenario: Algebraic enum payload rendering

- **WHEN** `Shape.Circle(radius: 5).to_string()` runs
- **THEN** the result is `Circle(radius: 5)`

#### Scenario: Local class surface name

- **WHEN** a class declared inside a function is rendered
- **THEN** the rendering uses its declared name without the `$local$N`
  suffix
