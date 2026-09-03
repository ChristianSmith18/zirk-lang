# zirk-callables Specification

## Purpose
Defines structural callable types, escaping closure environments, method
values, generators, and pipeline adaptation.
## Requirements
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

### Requirement: Generators, methods, and pipelines are ordinary callables
Naming a function SHALL produce its callable without address syntax. Bound methods SHALL retain their evaluated receiver, unbound methods SHALL expose it as the first parameter, partial application SHALL require an explicit lambda, `fn gen` SHALL return a lazy iterator with deterministic cleanup, and `value |> f(extra)` SHALL be equivalent to `f(value, extra)` without implicit Result/nullable propagation.

#### Scenario: Pipeline first argument
- **WHEN** `users |> paginate(page: 2)` is evaluated
- **THEN** it is checked as `paginate(users, page: 2)`

### Requirement: Callable values support polymorphic captures
The language SHALL allow the same `Fn(P...) => R` binding to hold differently-captured closures at different times without changing the static type.

#### Scenario: Two captured closures share a local
- **WHEN** `mut f: Fn() => Int32` is first assigned a closure capturing `a`, then a closure capturing `b`
- **THEN** both assignments type-check and calls dispatch to the correct closure at runtime

#### Scenario: Captured closure passed through a variable
- **WHEN** a captured closure is stored in a parameter, field, or local and later called
- **THEN** the call uses the stored closure's capture block

### Requirement: Escaping captured closures are boxed
The language SHALL lower captured closures that escape or are stored to a two-word `{function pointer, capture-block pointer}` representation, where the capture block is heap-allocated and GC-tracked.

#### Scenario: Returned closure keeps environment
- **WHEN** a function returns `Fn() => Int32` that captures a mutable counter
- **THEN** the returned callable owns a boxed capture block and repeated calls share the same counter

#### Scenario: Closure stored in a field
- **WHEN** a captured closure is stored in a `class` or `record` field
- **THEN** the capture block is heap-allocated and the field holds the two-word representation

### Requirement: Cloned callables produce independent environments
The language SHALL support `.clone()` on a callable when all captures are `Clone`, producing an independent deep copy of the capture block.

#### Scenario: Cloned counter is independent
- **WHEN** a callable is cloned and the original's captured counter is mutated
- **THEN** the clone's counter is unchanged

#### Scenario: Non-cloneable capture is rejected
- **WHEN** a callable captures a non-`Clone` resource and `.clone()` is called
- **THEN** compilation fails

