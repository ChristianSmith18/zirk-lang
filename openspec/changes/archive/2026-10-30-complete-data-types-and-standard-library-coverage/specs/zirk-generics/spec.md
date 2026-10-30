## ADDED Requirements

### Requirement: Generic enum dispatch preserves substituted type identity

A generic enum SHALL retain its concrete type arguments through construction, pattern matching, method or contract dispatch, runtime descriptors, and supported collection storage. Lowering SHALL dispatch each generic alternative using the exact substitution at the call site, without conflating distinct instantiations that have different observable layouts or behavior.

#### Scenario: Match a generic enum through a generic contract

- **WHEN** a `Result<T, E>`-like enum is passed through a generic contract and matched at a supported target
- **THEN** the selected payload has the substituted type and the same branch is selected by interpreter and native execution

#### Scenario: Distinct enum instantiations remain distinct

- **WHEN** two enum instantiations use different payload type arguments
- **THEN** dispatch, layout, and runtime type identity preserve the corresponding substitutions

### Requirement: User-defined generic function bodies and defaults are checked once

A user-defined generic function SHALL support a checked body, trailing defaulted type arguments, and supported defaulted value parameters. The checker SHALL validate the body against its declared constraints once, validate each default in the generic declaration context, and apply substitutions at instantiation without rechecking or erasing the generic contract. Inference and explicit arguments SHALL produce the same observable result when they resolve to the same substitution.

#### Scenario: Generic body is checked against the constraint

- **WHEN** a generic function body calls a member omitted from its constraints and is instantiated several times
- **THEN** one declaration-site diagnostic identifies the missing constraint

#### Scenario: Omitted generic argument uses its checked default

- **WHEN** a trailing generic type argument is omitted at a call site
- **THEN** its declared default is substituted and checked against its constraints

#### Scenario: Generic value default uses the instantiation

- **WHEN** a defaulted value parameter depends on a generic type parameter and is omitted
- **THEN** the default is checked once and evaluated with the concrete substitution for that call

### Requirement: Generic trait defaults use complete type substitution

A default method declared on a generic contract or trait SHALL be checked under its declared type parameters and SHALL be substituted consistently for receiver, arguments, result, inherited constraints, and nested generic applications at each adopter and call site.

#### Scenario: Invoke a default through two instantiations

- **WHEN** `Box<Int32>` and `Box<String>` inherit a default method from the same generic contract
- **THEN** each invocation observes its own substituted types and dispatches consistently through interpreter and native execution

### Requirement: Nested generic contract arguments are recursively substituted

Generic contract conformance and dispatch SHALL recursively substitute nested type expressions, including `Container<Box<T>>`, in required properties, method parameters, results, constraints, and defaults. A mismatch SHALL identify the fully substituted expected and actual types.

#### Scenario: Nested contract property conforms

- **WHEN** a type implements a contract requiring `Container<Box<T>>` for a concrete `T`
- **THEN** conformance checks every nested argument after substitution

#### Scenario: Nested contract dispatch uses the concrete substitution

- **WHEN** a method is called through a contract whose signature contains `Container<Box<T>>`
- **THEN** the implementation receives and returns values with the exact concrete nested types
