## REMOVED Requirements

### Requirement: Object and record spread
**Reason**: Spread currently depends on a nominal record/object shape, which conflicts with open dynamic objects.
**Migration**: Use object spread for dynamic properties and contracts for fixed typed views.

## ADDED Requirements

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
