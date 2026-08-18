# zirk-decorators Specification

## Purpose

Defines Zirk 1.x decorator declarations, targets, expansion phases, composition, repeatability, dependency validation, hygiene, permissions, generated API, diagnostics, caching, and compile-time erasure.

## Requirements

### Requirement: Decorators are compile-time declarations
Zirk SHALL declare non-repeatable decorators with `fn dec` and repeatable decorators with `repeatable fn dec`. A decorator SHALL be compile-time behavior rather than an ordinary runtime function value, and the compiler SHALL erase the decorator declaration and its applications after validated expansion.

#### Scenario: Decorator completes expansion
- **WHEN** a valid decorator application generates a wrapper and a static registry entry
- **THEN** the compiled program contains the validated wrapper and registry entry but no automatically retained decorator object or application metadata

### Requirement: Decorator targets are minimal and typed
Zirk 1.x SHALL support decorator target blocks only for `class`, `attribute`, `function`, `method`, and `parameter`. Parameter targets SHALL include initialization parameters. Method targets SHALL permit inspection of bodyless abstract, interface, and trait method signatures but SHALL permit wrapping only when an implementation body exists.

#### Scenario: Framework module is decorated
- **WHEN** `@Module(...)` is applied to a class
- **THEN** it is handled as a `class` target without introducing a language-level `module` target

#### Scenario: Unsupported constructor target is declared
- **WHEN** a decorator declares a `construct` target block
- **THEN** compilation fails and recommends class, attribute, or initialization-parameter generation instead

### Requirement: Expansion follows typed ordered phases
Each decorator target block SHALL expose an immutable typed target handle and SHALL select compiler operations through `match target.transform`. The compiler SHALL complete applicable `Inspect` phases before committing `Augment` phases and SHALL complete augmentation before `Wrap`. Generated syntax SHALL pass through normal name resolution, typing, effect analysis, and validation.

#### Scenario: Inspection rejects a target
- **WHEN** `Inspect(context)` emits an error for an invalid target
- **THEN** no augmentation or wrapper from that decorator is committed

### Requirement: Phase payloads are explicit
Every compiler-provided phase value SHALL be introduced by the matched variant payload or an explicit local binding. A decorator SHALL NOT receive undeclared implicit variables.

#### Scenario: Repeatable applications are inspected
- **WHEN** a repeatable decorator matches `Inspect(context, applications)`
- **THEN** both `context` and `applications` are explicit bindings originating from that match arm

### Requirement: Wrapper variants have precise behavior
Executable targets SHALL be wrapped through `match target.wrap` using `Before()`, `After(result)`, `After(result, transform)`, `Catch(error)`, or `Around(next)`. `Around` SHALL control the whole invocation and SHALL NOT coexist ambiguously with the other wrapper forms in one decorator application. A `Void` result SHALL use `After()`.

#### Scenario: Result is observed without replacement
- **WHEN** a decorator handles `After(result)`
- **THEN** it may inspect the result but cannot replace it

#### Scenario: Result is replaced once
- **WHEN** a decorator handles `After(result, transform)` and invokes `transform(new_value)` once with a compatible value
- **THEN** the wrapper returns the replacement and the transform capability cannot escape or be invoked again

### Requirement: Catch preserves explicit error semantics
`Catch(error)` SHALL permit exact rethrow, throwing another declared or inferred error, or recovery with a return-compatible value. A wrapper SHALL NOT remove a callable's `throws` behavior unless exhaustive analysis proves all applicable failures are recovered.

#### Scenario: Catch only handles one error case
- **WHEN** a decorated callable can produce multiple error variants and `Catch` recovers only one
- **THEN** the unhandled error variants remain visible in the callable's inferred failure contract

### Requirement: Variant patterns support explicit omission
Decorator phase and wrapper patterns SHALL use normal Zirk pattern-matching rules. `_` SHALL consume one payload position without binding it, multiple wildcards SHALL be allowed, and variant arity SHALL remain exact.

#### Scenario: Middle payload is ignored
- **WHEN** a three-payload variant is matched as `After(result, _, error)`
- **THEN** the middle position is consumed without creating a binding while `result` and `error` remain available

### Requirement: Wrappers preserve callable contracts
A decorator wrapper SHALL preserve target name, visibility, parameters, labels, defaults, variadics, generics, constraints, return type, receiver mutability, override slot, documentation, source map, and callable identity. Additional errors and permissions introduced by the wrapper SHALL remain visible to normal inference and diagnostics.

#### Scenario: Wrapper performs authorized file access
- **WHEN** a wrapper adds a file-reading operation
- **THEN** the resulting callable retains its original signature and its permission effect includes the inferred filesystem requirement

### Requirement: Decorator composition follows source order
Decorator expressions SHALL be evaluated top-to-bottom, and the closest decorator to the declaration SHALL be the innermost wrapper. For `@Outer` above `@Inner`, invocation SHALL compose as `Outer(Inner(target))`.

#### Scenario: Authorization precedes cache access
- **WHEN** `@Authorized` is written above `@Cached` on a callable
- **THEN** the authorization wrapper is outermost and receives the invocation before the cache wrapper

### Requirement: Dependency clauses validate visible composition
A decorator MAY declare `requires decorators [...]`, `before decorators [...]`, and `after decorators [...]` constraints involving different decorators. The compiler SHALL validate presence and source order and SHALL NOT silently reorder applications.

#### Scenario: Source order contradicts before constraint
- **WHEN** `@Authorized` declares `before decorators [Cached]` but is written below `@Cached`
- **THEN** compilation fails and suggests moving `@Authorized` above `@Cached`

#### Scenario: Required decorator is absent
- **WHEN** `@OpenApi` declares `requires decorators [Route]` and no `@Route` applies to the target
- **THEN** compilation fails at the `@OpenApi` application and identifies the missing dependency

### Requirement: Self-dependencies and cycles are invalid
A decorator SHALL NOT name itself in `requires`, `before`, or `after`. The compiler SHALL reject direct and indirect cycles across decorator dependency and ordering constraints and SHALL report the complete cycle.

#### Scenario: Repeatable decorator orders itself
- **WHEN** `Middleware` declares `before decorators [Middleware]`
- **THEN** its declaration fails and directs same-decorator ordering to grouped application inspection

#### Scenario: Indirect ordering cycle exists
- **WHEN** constraints imply `First -> Second -> Third -> First`
- **THEN** compilation fails and reports that complete cycle

### Requirement: Repeatable decorators use grouped ordered applications
Contiguous applications of a `repeatable fn dec` SHALL be processed as one logical expansion in source order rather than as independently nested copies. Each phase that consumes the group SHALL bind `applications` explicitly in its variant payload. The collection SHALL be immutable and typed for that decorator.

#### Scenario: Three middleware applications are grouped
- **WHEN** three contiguous `@Middleware(...)` applications decorate one function
- **THEN** one expansion receives three ordered application records matching their source order

#### Scenario: Repeated applications are separated
- **WHEN** applications of one repeatable decorator are separated by a different decorator
- **THEN** compilation fails and explains that repeatable applications must be contiguous

### Requirement: Repeatable arguments are accessed through application records
Parameters in a repeatable decorator declaration SHALL define the typed argument schema of each application. Inside grouped expansion, concrete values SHALL be accessed through an explicitly bound application record such as `application.arguments.name`; no single ambiguous parameter value SHALL be injected into the group body.

#### Scenario: Application argument is iterated
- **WHEN** `for application in applications` executes for `@Middleware("auth")`
- **THEN** `application.arguments.name` has the declared `String` type and value `"auth"`

### Requirement: Repeatable policy belongs to the decorator
The repeatable decorator implementation SHALL decide whether equal applications are combined, deduplicated, warned about, or rejected, and SHALL validate minimum or maximum cardinality through the explicit `applications` payload rather than a self-dependency clause.

#### Scenario: Duplicate roles are ignored by policy
- **WHEN** a repeatable role decorator defines exact-duplicate deduplication and receives the same role twice
- **THEN** it produces one logical role entry without changing the general language rule for other repeatable decorators

### Requirement: Generated names are hygienic and public conflicts fail
Private generated bindings SHALL have compiler identities that cannot collide with user bindings. Public declarations and members SHALL require explicit augmentation operations, and any public collision between generated or user declarations SHALL be a compile-time error without implicit priority or last-writer-wins behavior.

#### Scenario: Two decorators generate the same public method
- **WHEN** two augmentations attempt to add an identical public member name to one class
- **THEN** compilation fails and identifies both decorator applications and the conflicting member

### Requirement: Transformations preserve user guarantees
Decorators SHALL NOT delete user declarations, silently rename them, reduce visibility, change existing public parameters or return types, remove errors or permissions, replace bodies without source mapping, or otherwise bypass type, mutability, ownership, permission, and safety checks. Additive members and contracts SHALL be validated like user-written declarations.

#### Scenario: Decorator attempts to reduce visibility
- **WHEN** an augmentation changes a public method to private
- **THEN** expansion is rejected before code generation

### Requirement: Expansion recursion is explicit and bounded
Generated declarations SHALL NOT inherit or acquire decorator applications implicitly. Explicitly generated applications SHALL enter a later bounded expansion round, and the compiler SHALL detect repeated structural expansion and dependency cycles before exhausting hard resource limits.

#### Scenario: Decorator regenerates itself indefinitely
- **WHEN** an expansion repeatedly generates the same decorated structure
- **THEN** compilation stops with an expansion-cycle diagnostic linked to every participating application

### Requirement: Decorator expansion is deterministic and incremental
The expansion cache key SHALL cover the decorator implementation and version, arguments, typed target, relevant configuration, permissions, all observed external build inputs, and transitive dependencies. Unchanged fingerprints SHALL reuse validated results, while changes SHALL invalidate only affected expansions.

#### Scenario: No decorator input changes
- **WHEN** a project rebuilds with matching decorator and observed-input fingerprints
- **THEN** the compiler reuses the validated expansion without executing it again

### Requirement: Decorator effects require build authority
Decorator arguments and expansion SHALL perform filesystem, environment, network, process, or other privileged effects only through authority granted for `during: build` or `both`. Dependencies SHALL only request authority, and every observed value SHALL become a tracked build input. Secrets SHALL NOT appear in generated public structure, IR, diagnostics, logs, or cache keys.

#### Scenario: Decorator reads an unapproved environment variable
- **WHEN** a decorator attempts an environment read absent from the application's approved build permissions
- **THEN** expansion stops before the read and the trusted permission workflow identifies the decorator and dependency path

### Requirement: Generic decorators expand once per declaration
A decorator on a generic declaration SHALL expand once against the generic parameters and constraints, after which the validated result SHALL specialize normally. Zirk 1.x SHALL NOT rerun the decorator separately for every specialization.

#### Scenario: Generic service has multiple concrete uses
- **WHEN** one decorated generic service is instantiated with several type arguments
- **THEN** its decorator expands once and its generated typed form participates in each normal specialization

### Requirement: Overrides do not inherit decorator applications
An inherited method that is not overridden SHALL retain its existing decorated wrapper. A newly overriding method SHALL NOT inherit decorator applications automatically and SHALL require explicit reapplication when the behavior is desired.

#### Scenario: Decorated method is overridden without annotation
- **WHEN** a subclass uses `override fn` without repeating its base method's decorator
- **THEN** the override contains no newly inherited wrapper while the base implementation remains decorated

### Requirement: Generated API and diagnostics retain provenance
Generated public API SHALL participate in documentation, interface emission, compatibility analysis, and language tooling. Diagnostics SHALL retain the generated declaration, decorator application, target, expansion path, and actionable original span. Decorators MAY emit errors, warnings, and notes but SHALL NOT suppress compiler diagnostics.

#### Scenario: Generated member fails type checking
- **WHEN** a decorator generates an invalid method body
- **THEN** the diagnostic identifies the generated method, originating decorator, decorated target, and relevant user source span

### Requirement: Runtime metadata requires explicit ordinary generation
Zirk 1.x SHALL NOT provide `runtime fn dec`, automatic decorator retention, or a general `Reflection.decorators(...)` API. A decorator that needs runtime information SHALL explicitly generate an ordinary typed descriptor, member, or registry subject to normal typing, visibility, documentation, compatibility, permission, and dead-code rules.

#### Scenario: HTTP framework needs route discovery
- **WHEN** route decorators expand successfully
- **THEN** they generate a typed static router registry and no runtime scan of decorator metadata is required


