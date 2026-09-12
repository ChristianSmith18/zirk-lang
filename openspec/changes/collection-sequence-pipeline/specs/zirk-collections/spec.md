## ADDED Requirements

### Requirement: Sequence pipeline on `List<T>` and `Array<T>`
`List<T>` and `Array<T>` SHALL provide `map(fn: (T): R): List<R>`,
`filter(fn: (T): Boolean): List<T>` (returning the same family as the
receiver's own concrete type, i.e. `Array<T>.filter` returns `Array<T>` and
`List<T>.filter` returns `List<T>`), `reduce(identity: R, combine: (R, T): R): R`,
`sum(): T` (only for a numeric element type), `count(): Int32`, `collect(): List<T>`,
and `for_each(fn: (T): Void): Void`. Each SHALL run eagerly, left to right,
over the receiver's elements at the time of the call.

#### Scenario: map produces a new list
- **WHEN** `[1, 2, 3].map((x: Int32): Int32 => x * 2)` is evaluated
- **THEN** the result is `[2, 4, 6]` and the original collection is unchanged

#### Scenario: filter preserves the receiver's own family
- **WHEN** `values: Array<Int32> = [1, 2, 3, 4]; values.filter((x: Int32): Boolean => x > 2)` is evaluated
- **THEN** the result has type `Array<Int32>` and value `[3, 4]`

#### Scenario: reduce folds left to right
- **WHEN** `["a", "b", "c"].reduce("", (acc: String, x: String): String => acc + x)` is evaluated
- **THEN** the result is `"abc"`

#### Scenario: sum requires a numeric element type
- **WHEN** `[1, 2, 3].sum()` is evaluated
- **THEN** the result is `6`, typed the same as the element type

#### Scenario: collect materializes a pipeline
- **WHEN** `rows.map(f).filter(g).collect()` is evaluated
- **THEN** it consumes once and produces a `List<T>` holding the pipeline's result

### Requirement: Collection query and mutation companions on `List<T>` and `Array<T>`
`List<T>` and `Array<T>` SHALL provide `contains(value: T): Boolean`,
`sort(): Void` (element type SHALL have a natural ordering; a compile-time
error otherwise, directing the caller to `sort_by`), `sort_by(cmp: (T, T): Int32): Void`,
`reverse(): Void`, `first(): T?`, and `last(): T?`. `List<T>` SHALL
additionally provide `pop(): T?`, removing and returning the last element.
`sort`/`sort_by`/`reverse` SHALL mutate the receiver in place and return
`Void`; `first`/`last`/`pop` SHALL return `null` when the receiver is empty,
never a controlled exception.

#### Scenario: contains reports membership
- **WHEN** `[1, 2, 3].contains(2)` is evaluated
- **THEN** the result is `true`

#### Scenario: sort mutates the receiver in place
- **WHEN** `mut values: List<Int32> = [3, 1, 2]; values.sort();` runs
- **THEN** `values` is `[1, 2, 3]`

#### Scenario: sort rejects an element type with no natural ordering
- **WHEN** a `List<SomeClassWithNoOrdering>`'s `sort()` (no comparator) is checked
- **THEN** compilation fails, directing the caller to `sort_by`

#### Scenario: first and last are nullable, not exceptions
- **WHEN** an empty `List<Int32>`'s `first()` and `last()` are evaluated
- **THEN** both are `null`, and no exception is thrown

#### Scenario: pop removes and returns the last element
- **WHEN** `mut values: List<Int32> = [1, 2, 3]; inmut popped = values.pop();` runs
- **THEN** `popped` is `3` and `values` is `[1, 2]`

### Requirement: Sequence pipeline on `Range<T>`
`Range<T>` SHALL provide the same sequence pipeline `List<T>`/`Array<T>`
provide (`map`, `filter`, `reduce`, `sum`, `count`, `for_each`) plus
`collect(): List<T>`, materializing the range's own values before running
the pipeline. `Range<T>` SHALL NOT provide the in-place mutation methods
(`sort`/`sort_by`/`reverse`/`pop`) — a range has no backing storage to
mutate.

#### Scenario: a range pipeline collects to a list
- **WHEN** `(1..=5).map((x: Int32): Int32 => x * x).collect()` is evaluated
- **THEN** the result is `[1, 4, 9, 16, 25]`, typed `List<Int32>`

### Requirement: Parity methods on `Tuple`
`Tuple` SHALL provide `to_string(): String` and `clone(): Tuple(...)`
(matching `List<T>`/`Array<T>`'s own `to_string`/`clone`), and its declared
arity SHALL be available as a compile-time constant. `Tuple` SHALL NOT
provide `map`/`filter`/`reduce`/`for_each`/`sum`/`count`/`collect` — its
elements are not of one uniform type, so no single-callback pipeline method
can be typed over them.

#### Scenario: a tuple's to_string matches its literal shape
- **WHEN** `(1, "x").to_string()` is evaluated
- **THEN** the result is a `String` describing the tuple's own elements, consistent with `List<T>`/`Array<T>`'s own `to_string` formatting
