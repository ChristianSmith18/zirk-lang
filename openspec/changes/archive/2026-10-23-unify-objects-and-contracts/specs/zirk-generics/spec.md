## REMOVED Requirements

### Requirement: Recursive generics and managed indirection
**Reason**: Its inline recursion example is a record and no longer represents the dynamic object model.
**Migration**: Retain recursive constraints and reject infinite inline recursion in remaining value aggregates; class and object references provide managed indirection.

### Requirement: Specialization during lowering
**Reason**: Its rationale depends on storing records inline.
**Migration**: Retain specialization for concrete generic instantiations without prescribing record layouts.

### Requirement: Generic contract substitution does not corrupt the vtable
**Reason**: This requirement explicitly includes record implementers.
**Migration**: Preserve generic class contract dispatch and stable vtable layouts.

### Requirement: Recursion cap on generic substitution
**Reason**: This requirement explicitly includes generic record definitions.
**Migration**: Guard recursive class, contract, and alias substitution with a controlled diagnostic.

### Requirement: Generic contract method lowering
**Reason**: Generic record adopters are removed and dynamic object conformance must be represented explicitly.
**Migration**: Use generic classes for statically checked generic conformance or dynamic `Object` values for runtime-checked conformance.

## ADDED Requirements

### Requirement: Generic contracts constrain structural properties and operations
Generic constraints SHALL accept `contract` declarations and SHALL preserve their substituted property and operation types at each use site. A statically known class or object shape SHALL be checked at compile time. A value whose static type is only `Object` SHALL be checked at runtime and SHALL raise a catchable contract violation if it does not satisfy the instantiated contract.

#### Scenario: Generic class satisfies a substituted contract
- **WHEN** `Box<Int32>` is passed to a generic function constrained by a contract requiring `value: T`
- **THEN** the checker substitutes `Int32` and validates the required property type

#### Scenario: Dynamic object fails a generic contract
- **WHEN** a dynamic object is passed where a runtime-checked `Contract<String>` is required but its property has an incompatible value
- **THEN** a catchable contract violation is raised before the constrained operation executes

### Requirement: Generic classes preserve contract dispatch layout
Generic class contract methods SHALL use substituted parameter and result types at every call site. Vtable method offsets SHALL remain stable across generic instantiations that share the same contract declaration.

#### Scenario: Generic class contract dispatch is stable
- **WHEN** `Box<Int32>` and `Box<String>` both implement the same generic contract
- **THEN** each dispatches to its own substituted implementation using the corresponding stable vtable layout

### Requirement: Generic recursive substitution is bounded
Generic substitution SHALL be guarded against infinite recursion. Mutually recursive generic classes, contracts, aliases, and constraints SHALL produce a controlled diagnostic instead of exhausting the compiler stack.

#### Scenario: Recursive generic alias
- **WHEN** `type Nested<T> = Box<Nested<T>>;` is written
- **THEN** a diagnostic is emitted before the compiler recurses without bound

### Requirement: Generic specialization remains representation-independent
Lowering SHALL produce one specialization for each concrete combination of type arguments actually used, and MAY share code only when ABI and observable semantics remain unchanged. This rule SHALL NOT require inline storage for dynamic objects.

#### Scenario: Repeated generic instantiation
- **WHEN** the same generic function is used multiple times with the same argument types
- **THEN** the compiler may reuse a single compatible specialization
