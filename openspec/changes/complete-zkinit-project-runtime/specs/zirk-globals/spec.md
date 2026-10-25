## Purpose

Defines typed application-wide state declared in `.zkinit`, including compiler inference for access, deterministic default construction, immutable replacement, and observable change subscriptions.

## ADDED Requirements

### Requirement: Applications declare typed globals

Only an application SHALL declare `globals`. Each name SHALL resolve to one declared type. A shorthand declaration such as `counter: UInt64` SHALL use the type's default value. An expanded declaration SHALL require `type` and MAY provide `default`, `from`, or `readonly`. `from` SHALL identify a project-relative source file for a custom type; when omitted, ordinary project and package type resolution SHALL apply. `readonly` SHALL default to `false`.

#### Scenario: Shorthand global uses the type default
- **WHEN** `.zkinit` declares `counter: UInt64`
- **THEN** `counter` has type `UInt64` and initializes to that type's default value

#### Scenario: Expanded custom type resolves from a source file
- **WHEN** `.zkinit` declares `settings` with `type: Settings` and `from: "./src/config.zrk"`
- **THEN** the compiler loads `Settings` from that project-relative source file even if the application entry does not import it
- **AND** files reachable only through that type source are checked as type dependencies without executing their file-scope statements
- **AND** the direct type source is rejected if it contains executable file-scope statements

#### Scenario: Package declares application globals
- **WHEN** a package manifest contains a `globals` block
- **THEN** manifest validation rejects the block as application-only

### Requirement: Global defaults are deterministic and type-correct

An explicit `default` SHALL type-check against the declared type. When no explicit value exists, initialization SHALL derive a deterministic default from the type. For a custom constructed type, the compiler SHALL select one unambiguous constructor whose parameters can all be supplied; each parameter SHALL use its declared default when present and otherwise the default of that parameter's type. The selected constructor body SHALL execute once as ordinary initialization code. Manifest initialization SHALL NOT execute the application's entry top-level body to derive a default. If a type, default, or unique constructor cannot be resolved, compilation SHALL report a global-specific diagnostic; an error raised by the selected constructor SHALL be reported as a startup initialization failure before imported module and entry bodies execute.

#### Scenario: Custom object defaults its fields by type
- **WHEN** a global has type `Person` whose constructor requires `name: String` without an explicit parameter default
- **THEN** the default instance receives the default `String` value for `name`

#### Scenario: Explicit default has the wrong type
- **WHEN** a global declares type `UInt32` and a string default
- **THEN** compilation reports a type mismatch at the global declaration

#### Scenario: Default cannot be derived
- **WHEN** a custom type has a constructor parameter whose type has no permitted default
- **THEN** compilation reports that the global cannot be initialized

#### Scenario: Constructor body initializes a global object
- **WHEN** a global is default-constructed through one uniquely eligible constructor
- **THEN** its constructor receives the derived parameter defaults and runs once before module initialization

#### Scenario: Several constructors can supply defaults
- **WHEN** more than one constructor is eligible for default construction of a global type
- **THEN** compilation reports an ambiguous global default instead of choosing by declaration order

### Requirement: Global access is inferred from literal manifest keys

`Global.get`, `Global.set`, and `Global.watch` SHALL accept a statically known literal key in the initial release. The compiler SHALL infer the value type from `.zkinit`; caller-provided generic arguments SHALL NOT override that type. `Global.set` SHALL accept only a value compatible with the declared type. An unknown key, a dynamic key, or an incompatible value SHALL produce a compile-time diagnostic. A `readonly: true` global SHALL remain readable but SHALL reject `Global.set` at compile time.

Types used by `Global.get` or `Global.set` SHALL support structural `Clone` semantics so callers and stored state cannot share mutable aliases. The compiler SHALL reject a get/set operation when the declared type contains a value that cannot be safely deep-cloned; this initial boundary includes mutable collections and identity-bearing types without a supported clone contract.

#### Scenario: Get infers the manifest type
- **WHEN** code reads `Global.get("settings")` and the manifest declares `settings` as `Settings`
- **THEN** the expression has type `Settings` without an explicit type argument

#### Scenario: Set rejects an incompatible value
- **WHEN** code sets a `UInt64` global to a `String`
- **THEN** the compiler reports the incompatible value type

#### Scenario: Read-only global permits reads
- **WHEN** a global is marked `readonly: true`
- **THEN** `Global.get` remains valid and `Global.set` is rejected

#### Scenario: Unknown or dynamic key is used
- **WHEN** a global API receives an undeclared or non-literal key
- **THEN** the compiler reports that the key cannot be resolved from the manifest

### Requirement: Global updates use immutable replacement and value equality

`Global.set` SHALL replace the stored value as one state transition. A value returned by `Global.get` SHALL NOT permit in-place mutation that changes stored global state without a `set`. A successful set whose new value is structurally equal to the stored value SHALL NOT create a change event. Equality SHALL compare deterministic value contents rather than object identity; a type without a supported equality contract SHALL produce a diagnostic when used with `Global.watch`.

#### Scenario: Equal replacement is not a change
- **WHEN** `Global.set` supplies a value structurally equal to the current value
- **THEN** the stored state remains equivalent and no watch event is emitted

#### Scenario: Mutation cannot bypass set
- **WHEN** code obtains a global object and attempts to mutate an aliased field directly
- **THEN** the stored global value does not change without an explicit replacement

### Requirement: Watch emits ordered previous and current values

`Global.watch<T>(key)` SHALL infer `T` from the manifest and return a managed `GlobalSubscription<T>`. Its `snapshot()` SHALL expose the value and revision captured atomically at registration. Its `next()` SHALL return the next `GlobalChange<T>` containing `previous: T`, `current: T`, and a monotonically increasing `revision`, or a typed `GlobalWatchError`; `close()` SHALL release the subscription. A subscription SHALL NOT emit an initial event. Events for one global SHALL be delivered in revision order and observer code SHALL run outside the global state lock.

Subscriptions SHALL close explicitly or when their owning scope ends. Delivery SHALL be bounded; overflow SHALL produce one terminal `GlobalWatchError.Lagged` and SHALL NOT silently drop events while claiming lossless delivery. After lag, the caller SHALL register a new subscription and use its atomic snapshot to resume.

#### Scenario: Watch reports a real replacement
- **WHEN** a global changes from `old` to a structurally different `new` value
- **THEN** the subscriber receives one event containing `previous: old`, `current: new`, and a higher revision

#### Scenario: Watch does not report initialization as a change
- **WHEN** a subscription is registered after global initialization and before any set
- **THEN** it emits no change event and exposes its current snapshot through the handle

#### Scenario: Slow subscriber reaches its bounded limit
- **WHEN** a subscriber cannot keep up with committed global changes
- **THEN** `next()` returns a terminal `GlobalWatchError.Lagged` and the subscription does not silently claim lossless delivery

### Requirement: Global initialization precedes source execution

All application globals SHALL initialize after manifest, type, and dependency validation but before any imported module or application entry top-level body executes. Initialization order SHALL be deterministic. A failure during resolution or default construction SHALL produce a compiler or startup diagnostic before the application entry runs.

#### Scenario: Imported module reads an initialized global
- **WHEN** an imported module reads a declared global during its top-level initialization
- **THEN** it observes the initialized default or configured value

#### Scenario: Global initialization fails
- **WHEN** a declared global cannot be resolved or constructed
- **THEN** startup reports the global failure and does not execute imported module or entry code
