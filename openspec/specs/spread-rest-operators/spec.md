# spread-rest-operators Specification

## Purpose
TBD - created by archiving change spread-rest-operators. Update Purpose after archive.
## Requirements
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

### Requirement: Dynamic object spread and rest
Object spread SHALL create a new open `Object` and copy the source's enumerable string-keyed properties in deterministic insertion order. Explicit properties SHALL override copied properties. Copying SHALL preserve contained value identity rather than deep-cloning nested reference values. Object rest SHALL create a new open `Object` containing the unselected keys. Object spread SHALL NOT require `Iterable<T>`.

#### Scenario: Spread copies keys and applies an override
- **WHEN** `{ ...profile, name: "Grace" }` is evaluated
- **THEN** it creates a distinct object with all source keys and the new `name` value

#### Scenario: Spread preserves nested object identity
- **WHEN** a source object contains a nested object and is spread
- **THEN** the result's corresponding property refers to the same nested object

#### Scenario: Object rest collects unselected keys
- **WHEN** `{ id, ...details }` destructures an object with `id`, `name`, and `active`
- **THEN** `details` is an open `Object` containing `name` and `active`

#### Scenario: Object spread is distinct from iterable spread
- **WHEN** an object is used in `{ ...source }`
- **THEN** named properties are copied without requiring `Iterable<T>`
