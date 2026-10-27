## MODIFIED Requirements

### Requirement: Singleton values widen only under explicit inference rules
Singleton values SHALL be assignable to their owning primitive or value type. Type inference SHALL preserve a singleton when required by an explicit singleton annotation, a singleton union alternative, or an immutable literal context. Mutable bindings SHALL widen to their declared base type unless a narrower invariant is proven. Contextual typing SHALL NOT silently widen away a discriminant required by the expected type. Singleton annotations SHALL use a type name or direct literal; temporal singleton values SHALL use only the owning `Date(...)`, `Time(...)`, or `DateTime(Date(...), Time(...))` constructor forms with direct integer components. Bare ISO temporal spellings, wrapper calls, arbitrary function calls, and computed expressions SHALL NOT be part of singleton type syntax.

#### Scenario: Literal fits its primitive type
- **WHEN** the singleton `404` is assigned to `Int32`
- **THEN** assignment succeeds if the exact value is representable by `Int32`

#### Scenario: Context preserves a union discriminant
- **WHEN** `{ active: true }` is checked against a union requiring `active: true` or `active: false`
- **THEN** contextual typing retains the true singleton while selecting the matching alternative

#### Scenario: Mutable variable widens safely
- **WHEN** `mut active = true` is declared without a singleton expectation
- **THEN** its inferred type permits later assignment of `false`

#### Scenario: Temporal constructor singleton retains its owning type
- **WHEN** the Date singleton `Date(2026, 12, 25)` is assigned to `Date`
- **THEN** assignment succeeds while its explicit singleton annotation remains exact

#### Scenario: Wrapper expression is not a type annotation
- **WHEN** a type annotation contains `const(1)`, `Date.today()`, or another arbitrary call
- **THEN** the parser rejects it and directs the author to use a type name, a direct non-temporal literal, or a fixed temporal constructor literal
