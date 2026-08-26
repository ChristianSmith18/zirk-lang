## MODIFIED Requirements

### Requirement: Runtime type identity is safe and representation-independent
Every value and declared type MUST expose compiler-provided `Type` identity.
Identity SHALL distinguish constructed generic arguments and provide broad
`TypeKind`, safe names, and declared implements/extends relationships without
exposing layout, private members, or optimized representation. Value type
observation and checked casts SHALL remain separate operations.

#### Scenario: Interface value contains a concrete class
- **WHEN** `value` is declared as an interface and contains an implementing class instance
- **THEN** `value.type()` identifies the concrete class
- **AND** the interface's own kind remains available through `InterfaceName.type()`

#### Scenario: Constructed generic types share optimized code
- **WHEN** two constructed generic types reuse an implementation representation
- **THEN** their `Type` identities remain distinct when their type arguments differ

#### Scenario: A user-declared generic enum instantiates like a compiler-native one
- **WHEN** a program declares `enum Bar<T> { ... }` and instantiates it as `Bar<Int32>`
- **THEN** the instantiation lowers to its own concrete layout, the same mechanism already used for `Result<T,E>`

NOTE (not applied to the main spec): this change's own implementation found that a self-referencing enum declaration — recursive, generic or not — cannot be declared at all today, independent of genericity (`declare_enum` resolves every variant's field types before the enum's own name is registered). A "recursive generic enum terminates" scenario was originally planned here but is not delivered; it belongs to a future change that first gives enum declaration the same two-phase declare/resolve split classes already have.
