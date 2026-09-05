# zirk-collections delta

## ADDED Requirements

### Requirement: Collection literal construction

`Array<T>` SHALL be constructible from an element listing
(`Array(e0, e1, …)` or `[e0, e1, …]` with an `Array<T>` context), and
`List<T>` from `List(e0, e1, …)`, both inferring or checking `T` against
the destination.

#### Scenario: Pre-populated list

- **WHEN** `mut l: List<Int32> = List(10, 20)` is constructed
- **THEN** `l.length` is `2` and `l[0]`/`l[1]` read `10`/`20`

#### Scenario: Array literal in context

- **WHEN** `inmut a: Array<Int32> = [1, 2, 3]` is constructed
- **THEN** `a.length` is `3`

### Requirement: Negative indexing counts from the end

`Array<T>`, `List<T>`, and `String` index/slice reads and writes SHALL
accept negative indices, resolved as `length + index`.

#### Scenario: Last element

- **WHEN** `a[-1]` is read on a three-element array
- **THEN** it yields the third element

#### Scenario: Still bounds checked

- **WHEN** `a[-4]` is read on a three-element array
- **THEN** a controlled bounds error is raised

### Requirement: Collection clone and rendering

`Array<T>` and `List<T>` SHALL expose `clone()` producing an independent
collection and `to_string()` producing a readable rendering.

#### Scenario: Clone separates

- **WHEN** `l.clone()` is taken and the clone is mutated
- **THEN** the source list is unchanged

#### Scenario: Rendering

- **WHEN** `List(1, 2).to_string()` runs
- **THEN** the result renders both elements

### Requirement: Array slicing

`Array<T>` SHALL support `a[start:end:step]` producing a new `Array<T>`
copy in safe code, with the same bound/step rules as `String` slicing.

#### Scenario: Middle slice

- **WHEN** `a[1:4]` is taken from a five-element array
- **THEN** the result is an `Array<T>` of three copied elements

### Requirement: List value removal

`List<T>` SHALL expose `remove(value)` removing the first equal element
and returning `Boolean` when `T` supports equality, alongside the existing
`remove(index)`.

#### Scenario: Value removal

- **WHEN** `List(1, 2).remove(2)` runs
- **THEN** it returns `true` and the list holds `[1]`
