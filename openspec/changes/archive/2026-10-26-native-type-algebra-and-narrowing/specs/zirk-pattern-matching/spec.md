## ADDED Requirements

### Requirement: Matching narrows composite and singleton types
Each match arm SHALL receive the type remaining after its pattern is applied to the scrutinee. Supported narrowing SHALL include union alternatives, singleton values, enum variants and payloads, structural object-property patterns, nullability, and compatible intersections. Duration literals and deterministic constructor-call patterns SHALL be evaluated by the bounded constant evaluator; effectful or non-constant calls SHALL be rejected. An impossible pattern SHALL be diagnosed or marked unreachable. Exhaustiveness SHALL be computed from the remaining canonical type, not from source spelling or alias order.

#### Scenario: Union member pattern narrows its binding
- **WHEN** a match arm selects one member type of a union
- **THEN** the arm's binding and member accesses use only that selected type

#### Scenario: Literal alternatives are exhaustive
- **WHEN** a match covers every member of a finite singleton union
- **THEN** the compiler accepts the match as exhaustive

#### Scenario: Date singleton alternatives are exhaustive
- **WHEN** a match covers each member of a finite Date singleton union
- **THEN** the compiler accepts the match and diagnoses any missing canonical Date value

#### Scenario: Mixed finite unions include every category
- **WHEN** a finite union combines singleton values and enum variants
- **THEN** exhaustiveness checks every canonical value and qualified enum constructor in the normalized union

#### Scenario: Nullable finite domains include null
- **WHEN** a Boolean or finite singleton union is nullable
- **THEN** every finite value and the `null` case must have an arm unless a wildcard covers the remainder

#### Scenario: Temporal constructor pattern matches by canonical value
- **WHEN** a Date, Time, or DateTime constructor pattern is compared with a runtime value
- **THEN** both interpreter and native execution compare the owning type's canonical scalar value

#### Scenario: Structural discriminants narrow alternatives
- **WHEN** a match tests a literal property that uniquely distinguishes structural union alternatives
- **THEN** the selected arm is typed as the matching structural alternative

#### Scenario: Binding pattern receives the remaining alternatives
- **WHEN** earlier literal or required-property patterns consume alternatives from a finite union
- **THEN** a later binding receives only the remaining union, singleton, or structural shape

#### Scenario: Duplicate and impossible arms are diagnosed
- **WHEN** an arm repeats a canonical finite case or no remaining alternative can match its pattern
- **THEN** the compiler reports an unreachable-arm diagnostic and continues checking the arm body for independent errors

#### Scenario: Control-flow joins keep only live predecessor facts
- **WHEN** `if`, `match`, `try`/`catch`, or a loop reaches a merge from multiple paths
- **THEN** the resulting type joins facts from paths that can continue and excludes paths that return, throw, or diverge

#### Scenario: Loop facts converge within a fixed budget
- **WHEN** a loop condition and body transfer flow facts around a back edge
- **THEN** the checker iterates the monotone join to a stable header or emits the deterministic flow-iteration budget diagnostic

#### Scenario: Impossible alternative is reported
- **WHEN** a pattern cannot match any member of the narrowed scrutinee type
- **THEN** a diagnostic identifies the impossible pattern

### Requirement: Control-flow predicates narrow stable places soundly
Conditions using literal equality, type tests, property-presence tests, Boolean composition, negation, and declared type predicates SHALL refine stable local values and supported property paths. Refinements SHALL join at control-flow merges, converge at loops under a fixed budget, and be invalidated by writes, mutable aliases, or calls that may change the refined path. User-declared predicates SHALL be checked against the stated subtype relationship and SHALL NOT forge arbitrary refinements.

#### Scenario: Boolean discriminant narrows each branch
- **WHEN** if state.active tests a Boolean singleton discriminant
- **THEN** the true and false branches receive their corresponding union alternatives

#### Scenario: Equality narrows a literal union
- **WHEN** if code == 404 tests a finite numeric singleton union
- **THEN** the matching branch receives the 404 singleton and the other branch excludes it

#### Scenario: Structural discriminants narrow the root union
- **WHEN** a property such as `state.active` is compared with a singleton value declared differently by each structural alternative
- **THEN** the matching branch narrows both the projected property and the root value to compatible alternatives

#### Scenario: Property presence narrows known structural alternatives
- **WHEN** a string literal key is tested with `key in value` against a union of closed structural alternatives
- **THEN** known required and absent properties remove impossible alternatives while optional properties remain possible on both branches

#### Scenario: Open object presence remains runtime-only
- **WHEN** a key is tested on an open `Object` or `Dynamic` value
- **THEN** the compiler checks runtime presence without inventing a static property type or treating the test as exhaustive

#### Scenario: Nominal type tests and checked casts narrow values
- **WHEN** a local is tested with `value is Box` or `(value as? Box) != null`
- **THEN** a successful branch uses `Box`, an unrelated nominal test is diagnosed as impossible, and Dynamic payload extraction uses the exact runtime tag that was checked

#### Scenario: Property mutation invalidates a refinement
- **WHEN** a mutable discriminant property is changed inside a narrowed region
- **THEN** later operations use the widened declared type unless the compiler proves the new value

#### Scenario: Unknown mutating call invalidates an escaped path
- **WHEN** a narrowed mutable object escapes to a call that may mutate it
- **THEN** refinements of affected properties are discarded after the call

#### Scenario: Direct safe type test defines a predicate
- **WHEN** a Boolean function's only return statement is `return value is Target` for one of its parameters
- **THEN** calls to that function narrow the corresponding argument on true, while a function with an impossible test reports the ordinary impossible-type diagnostic

#### Scenario: Native Boolean results do not invent predicates
- **WHEN** an extern Boolean function is used as a condition
- **THEN** its arguments keep their declared types unless Zirk code performs its own checked type test

#### Scenario: Loop refinement converges
- **WHEN** a loop repeatedly changes a narrowed value
- **THEN** analysis reaches a stable conservative type or reports the deterministic flow-analysis budget diagnostic
