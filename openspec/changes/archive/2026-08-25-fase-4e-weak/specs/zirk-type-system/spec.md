## ADDED Requirements

### Requirement: `Weak<T>` is restricted to a reference-typed referent

`Weak<T>` SHALL only be constructed or named when `T` is a reference type (a class or contract instance) with observable identity — a value type (record, value class, or scalar) SHALL be rejected, since it has no identity for a weak reference to observe independently of its content.

#### Scenario: Disallowed value-type referent
- **WHEN** an annotation names `Weak<Int32>` or `Weak<SomeRecord>` where `SomeRecord` is a record type
- **THEN** compilation rejects it, naming the disallowed referent type

### Requirement: `Weak<T>` construction and observation

`Weak.from(value: T): Weak<T>` SHALL construct a weak handle to `value`'s referent without requiring an unsafe boundary. `.upgrade(): T?` SHALL return the referent when it is still reachable through some strong reference, and `null` otherwise. `.is_alive: Boolean` SHALL report the same underlying state as an observation, without itself producing a strong reference.

#### Scenario: Upgrade while the referent is alive
- **WHEN** `.upgrade()` is called while a strong reference to the same object is still reachable
- **THEN** it returns that object

#### Scenario: Is-alive is observational only
- **WHEN** `.is_alive` is read
- **THEN** it reports the current liveness of the referent without extending its lifetime
