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

#### Scenario: A recursive generic enum terminates
- **WHEN** a generic enum's own variant payload references the enum's own generic type (for example `Tree<T>` with a `Node(T, Tree<T>, Tree<T>)` variant)
- **THEN** specialization completes without infinite recursion, reusing one layout per distinct instantiation
