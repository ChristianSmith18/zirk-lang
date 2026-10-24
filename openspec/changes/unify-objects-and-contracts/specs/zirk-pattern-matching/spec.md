## REMOVED Requirements

### Requirement: Pattern forms and reachability
**Reason**: This requirement includes nested record patterns and assumes records are closed value aggregates.
**Migration**: Use object property patterns for dynamic values and contracts for statically declared shapes.

### Requirement: Pattern binding copy semantics
**Reason**: Direct record destructuring and implicit record copying are removed.
**Migration**: Match dynamic object properties by key; nested reference values preserve identity unless explicitly cloned.

## ADDED Requirements

### Requirement: Dynamic object patterns are partial tests
Object property patterns SHALL test key presence and match the selected values without mutating the source. Because `Object` is open, an object pattern SHALL NOT make a match exhaustive; a wildcard or other exhaustive fallback SHALL be required. Missing keys SHALL make that pattern fail rather than raise a missing-property error.

#### Scenario: Object pattern matches present keys
- **WHEN** an object with `name` and `active` is matched against `{ name: value }`
- **THEN** the pattern matches and binds the `name` value

#### Scenario: Object pattern with a missing key fails safely
- **WHEN** an object without `name` is matched against `{ name: value }`
- **THEN** the pattern does not match and no runtime error is raised

#### Scenario: Object patterns require a fallback
- **WHEN** a match over `Object` contains only property patterns and no wildcard
- **THEN** the checker reports that the match is not exhaustive

### Requirement: Dynamic property bindings preserve reference identity
When a dynamic object pattern binds a reference-valued property, the binding SHALL refer to the same object identity. Scalar values SHALL retain their ordinary value semantics. A binding whose type remains `Dynamic` SHALL require safe narrowing before concrete use.

#### Scenario: Nested object binding keeps identity
- **WHEN** an object pattern binds a nested object property and that nested object is mutated
- **THEN** the mutation is visible through both references

### Requirement: Tuple and enum pattern bindings retain value semantics
Tuple elements and enum payloads SHALL retain their existing pattern binding and copy rules. Dynamic object patterns SHALL use the reference-preserving rules defined for object properties and SHALL NOT change tuple or enum exhaustiveness.

#### Scenario: Enum payload pattern remains exhaustive
- **WHEN** a match destructures every variant of an algebraic enum
- **THEN** the checker verifies exhaustive coverage and binds each payload using its declared type
