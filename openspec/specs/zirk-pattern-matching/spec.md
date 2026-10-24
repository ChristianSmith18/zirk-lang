# zirk-pattern-matching Specification

## Purpose
Defines exhaustive guard-free matching, supported pattern families, binding
copy semantics, reachability, and match-expression typing.
## Requirements
### Requirement: Exhaustive guard-free matching
Every statement or expression match over a closed domain SHALL be exhaustive. Patterns SHALL NOT contain guards. Expression branch results SHALL unify with Never compatible with every result; statement match SHALL produce Void.

#### Scenario: Missing statement enum case
- **WHEN** a statement match omits one closed enum case and has no wildcard
- **THEN** compilation fails with the missing case identified

#### Scenario: Guard rejected
- **WHEN** a match branch appends `if condition` to a pattern
- **THEN** parsing fails and conditional logic must be placed in the branch body

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
