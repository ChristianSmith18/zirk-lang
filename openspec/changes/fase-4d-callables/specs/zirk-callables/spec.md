## MODIFIED Requirements

### Requirement: Writable structural callable types

The language SHALL recognize `Function(P...) => R` as the native callable type and `Fn(P...) => R` as its preferred exact alias. Named functions, contextually typed lambdas, bound/unbound methods, and explicit callable-contract values SHALL adapt when parameter labels/options/variadics, parameter types, result, mutability, safety, and permissions are compatible; parameters SHALL be contravariant and results covariant.

#### Scenario: Preferred callable alias

- **WHEN** `mut print: Fn(String) => Void = stdout.println` is declared
- **THEN** `print` is a bound callable to the same `stdout` receiver and MAY later be rebound

#### Scenario: Result remains the error contract

- **WHEN** a fallible callback is annotated `Fn(String) => Result<User, ParseError>`
- **THEN** expected failure is represented by the result and no separate callable throws syntax is required

#### Scenario: Recursive lambda with an explicit binding type

- **WHEN** a lambda calls itself by referring to the name it is bound to, and that name carries an explicit `Fn(...) => R` annotation
- **THEN** the recursive call type-checks and runs correctly

### Requirement: Escaping closures and capture environments

Closures SHALL be passable, returnable, storable, and collectable. Immutable value captures SHALL snapshot logical values; whole-reference captures SHALL share the referent; written mutable captures SHALL share one compiler-managed cell among closures. Assignment SHALL share the callable environment, while `clone()` SHALL produce an independent deep environment when all captures are cloneable. Callable identity SHALL be compared with `is`; callables SHALL NOT implement `==`.

#### Scenario: Escaping counter

- **WHEN** a function returns `Fn() => Int32` that increments a captured mutable counter
- **THEN** the counter survives its creating frame and repeated calls observe the shared lifted cell

#### Scenario: Projected capture

- **WHEN** a closure body captures `user.name` and `user.name` later changes
- **THEN** the closure observes the independent value captured at creation rather than the later attribute value

#### Scenario: Callable identity

- **WHEN** two callable values are compared with `is`
- **THEN** the comparison is valid and answers whether they are the same underlying callable with the same captured state
