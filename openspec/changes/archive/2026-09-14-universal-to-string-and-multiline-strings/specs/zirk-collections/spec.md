# zirk-collections Delta

## MODIFIED Requirements

### Requirement: Collection clone and rendering

`Array<T>` and `List<T>` SHALL expose `clone()` producing an independent
collection and `to_string()` producing a readable `[e0, e1, …]` rendering.
`Map<K, V>`, `Set<T>` and `Range<T>` SHALL expose `to_string()` producing
the `{k: v, …}`, `{e, …}` and literal-mirroring renderings defined by the
`universal-string-rendering` capability. All collection types SHALL be
valid arguments to `print`/`println` and `{expr}` interpolation.

#### Scenario: Clone separates

- **WHEN** `l.clone()` is taken and the clone is mutated
- **THEN** the source list is unchanged

#### Scenario: Rendering

- **WHEN** `List(1, 2).to_string()` runs
- **THEN** the result is `[1, 2]`

#### Scenario: Map rendering

- **WHEN** a `Map<Int32, String>` holding `{2: "b", 1: "a"}` is rendered
- **THEN** the result is `{1: a, 2: b}` — deterministically ordered

#### Scenario: Range rendering

- **WHEN** `(1..=10:2).to_string()` runs
- **THEN** the result is `1..=10:2`

#### Scenario: Collections print directly

- **WHEN** `println(list)` or `"items: {list}"` is written with a
  `List<Int32>`
- **THEN** the check succeeds and the `[…]` rendering is produced
