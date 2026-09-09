## ADDED Requirements

### Requirement: Explicit spread in calls

The language SHALL accept `...expression` as a positional call argument when `expression` implements `Iterable<T>`. It SHALL expand elements in iteration order, exactly once, before matching fixed and variadic parameters.

#### Scenario: Spread list into a variadic call
- **WHEN** `sum(...values)` is called with `values: List<Int32>` containing `[1, 2, 3]`
- **THEN** the callee receives three positional arguments and returns the same result as `sum(1, 2, 3)`

#### Scenario: Spread range into a variadic call
- **WHEN** `sum(...(0..3))` is called
- **THEN** the callee receives `0`, `1`, and `2` in that order

### Requirement: Explicit spread in collection literals and constructors

Array/List literals and `Array(...)`/`List(...)` calls SHALL accept spread elements/arguments from `Iterable<T>`, expanding them in place with scalar elements and preserving source order.

#### Scenario: Mixed collection spread
- **WHEN** `[0, ...values, 4]` is constructed from `[1, 2, 3]`
- **THEN** the resulting collection contains `[0, 1, 2, 3, 4]`

#### Scenario: Empty spread
- **WHEN** `List(...empty_values)` is constructed from an empty iterable
- **THEN** the result is an empty list without an error

### Requirement: Rest destructuring

An ordered collection pattern MAY contain one `...name` rest binding, which SHALL be last and SHALL collect all elements after the fixed prefix without mutating the source.

#### Scenario: Rest collects the suffix
- **WHEN** `[first, ...remaining]` destructures `[10, 20, 30]`
- **THEN** `first` is `10` and `remaining` contains `[20, 30]`

#### Scenario: Empty rest suffix
- **WHEN** `[first, ...remaining]` destructures `[10]`
- **THEN** `remaining` is an empty collection

### Requirement: Object and record spread

Record/object expressions SHALL support field-based spread using `{ ...source, field: value }` when an expected nominal record/object shape is available. Spread fields SHALL be copied in source order and explicit fields SHALL override earlier fields. Object spread SHALL NOT require `Iterable<T>` and SHALL NOT be interchangeable with collection spread.

#### Scenario: Record spread with override
- **WHEN** `{ ...profile, name: "Grace" }` is evaluated as a `UserProfile`
- **THEN** all fields are copied from `profile` and only `name` is replaced

#### Scenario: Record rest destructuring
- **WHEN** `{ id, ...details }` destructures a `UserProfile`
- **THEN** `id` receives the selected field and `details` contains every remaining named field

#### Scenario: Object spread requires a shape
- **WHEN** `{ ...value }` is written without a record/object expected type
- **THEN** the checker emits a diagnostic instead of creating an untyped dictionary
