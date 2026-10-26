## ADDED Requirements

### Requirement: Generic type aliases accept complete type expressions
Type aliases SHALL accept type parameters, constraints, defaults, and complete nested type expressions in their target. Alias substitution SHALL preserve canonical union/intersection identity and source alias names for diagnostics. Recursive aliases SHALL be accepted only when guarded by a value-bearing type constructor and SHALL terminate under the type-computation budget.

#### Scenario: Generic alias substitutes a union
- **WHEN** type Outcome<T, E> = Success<T> | Failure<E> is instantiated
- **THEN** each parameter is substituted in its corresponding alternative

#### Scenario: Defaulted alias parameter
- **WHEN** a generic alias omits a trailing type argument with a declared default
- **THEN** that default is substituted and checked against its constraint

#### Scenario: Unguarded recursive alias is rejected
- **WHEN** an alias recursively names itself without a guarding constructor
- **THEN** compilation reports a recursive-type diagnostic rather than recursing indefinitely

### Requirement: Type operators provide structural queries and transformations
The type language SHALL provide keyof, indexed access, conditional types with infer, mapped types with key remapping and optional/read-only modifier control, and finite template-literal types with intrinsic upper/lower/capitalize/uncapitalize transformations. Operators SHALL compose with generic parameters, unions, intersections, aliases, tuples, and structural object types. Distributive conditional behavior SHALL be explicit and deterministic. Type-level operators SHALL be erased before runtime and SHALL NOT permit access to private or otherwise invisible members.

#### Scenario: Key query returns literal keys
- **WHEN** keyof is applied to a structural type with public properties
- **THEN** it produces a normalized union of those property-name singleton types

#### Scenario: Indexed access distributes over key unions
- **WHEN** T[K] uses a finite union of valid keys
- **THEN** it produces the normalized union of the corresponding property types

#### Scenario: Conditional type distributes over a naked parameter
- **WHEN** a distributive conditional type receives a union argument
- **THEN** the conditional is evaluated for each member and the results are normalized

#### Scenario: Conditional inference binds a matched component
- **WHEN** a conditional type uses infer to match a generic component
- **THEN** the inferred type is scoped to the true branch and satisfies all constraints

#### Scenario: Mapped type preserves or changes modifiers
- **WHEN** a mapped type transforms a structural type's properties
- **THEN** key selection, remapping, optionality, read-only state, and value transformation follow the written modifiers

#### Scenario: Template literal type expands finite alternatives
- **WHEN** a template-literal type interpolates finite string, numeric, Boolean, or other string-convertible singleton unions
- **THEN** the compiler produces the finite normalized string union within its expansion budget

#### Scenario: Intrinsic string transformation maps singleton unions
- **WHEN** Uppercase, Lowercase, Capitalize, or Uncapitalize is applied to a finite String singleton union
- **THEN** each value is transformed according to the documented Unicode policy and the results are normalized

#### Scenario: Private member is not exposed by type operators
- **WHEN** keyof or indexed access is evaluated outside the declaring visibility scope
- **THEN** inaccessible members are excluded or rejected under ordinary visibility rules

### Requirement: Standard type utilities are available without runtime code
The standard type prelude SHALL provide generic aliases for partial/required/read-only properties, property selection/omission/record construction, union extraction/exclusion, null removal, and callable parameter/result extraction, plus task-result unwrapping where a Zirk task type applies. Utilities SHALL preserve generic constraints, modifiers, and visibility and SHALL be expressible by the public type operators.

#### Scenario: Utility type selects properties
- **WHEN** Pick<T, K> is applied to a structural type
- **THEN** the result contains exactly the selected accessible properties with their declared types

#### Scenario: Utility type builds a record shape
- **WHEN** Record<K, V> is applied to a finite singleton key union
- **THEN** the result has exactly those keys with value type V

#### Scenario: Union utility excludes members
- **WHEN** Exclude<T, U> is applied to a finite union
- **THEN** members assignable to U are removed while remaining members preserve their canonical identity

#### Scenario: Callable utility extracts a result
- **WHEN** ReturnType<F> is applied to a supported callable type
- **THEN** it resolves to the callable's checked result type

#### Scenario: String utility transforms literal keys
- **WHEN** an intrinsic case utility is applied to a finite string singleton union
- **THEN** it returns the corresponding finite transformed singleton union

### Requirement: Generic inference composes union and intersection constraints
Generic inference SHALL collect constraints from arguments, receivers, expected results, callable contexts, and type operators. It SHALL solve union and intersection constraints without arbitrary first-match selection, honor declared variance, preserve literal information when required by context, and issue a focused diagnostic when several incompatible solutions remain.

#### Scenario: Inference combines argument and expected result
- **WHEN** a generic function's argument and expected result contribute compatible constraints on T
- **THEN** the checker selects the unique canonical solution

#### Scenario: Inference rejects ambiguous candidates
- **WHEN** equally valid candidates imply incompatible substitutions
- **THEN** the call reports ambiguity and identifies the conflicting constraints

#### Scenario: Mutable generic remains invariant
- **WHEN** a union or intersection argument would require unsound covariance through a mutable generic container
- **THEN** assignment is rejected under declared variance rules
