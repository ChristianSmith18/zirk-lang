## ADDED Requirements

### Requirement: Composite runtime acceptance matches the supported representation matrix
Semantic analysis SHALL admit a composite type in executable code only when its runtime value representation and every operation used by that program are supported through verified IR, the interpreter, native code generation, managed-reference tracing, and any crossed compiled-package boundary. The executable union matrix SHALL cover all supported scalar families (including `Float128`, `Decimal`, and wide integers), temporal and text values, dataless and payload enums, tuples and records, structural shapes, nominal object references, collection references, nullable alternatives, and nested normalized unions. A representation absent from the matrix SHALL receive a stable source-located diagnostic before backend emission; it SHALL NOT be accepted and fail later with an unsupported-operation fallback or backend panic. The 4096-byte inline payload limit MAY remain, provided over-limit alternatives produce a deterministic diagnostic or use a verified managed fallback.

#### Scenario: Supported runtime families form executable unions
- **WHEN** a program forms unions from supported scalar, temporal, text, enum, tuple/record, shape, nominal-reference, collection-reference, nullable, or nested-union alternatives
- **THEN** each alternative retains its identity and value through construction, assignment, parameter passing, return, local or aggregate storage, capture, narrowing, and exhaustive matching

#### Scenario: Collection references can be union alternatives
- **WHEN** a union alternative is a `Map`, `Set`, `Array`, `List`, `Deque`, `PriorityQueue`, `Queue`, or `Stack` reference
- **THEN** union injection, narrowing, projection, and managed-reference tracing preserve the same collection identity and generic arguments

#### Scenario: Unsupported union alternative is diagnosed at source
- **WHEN** a type with no supported executable runtime representation is used as a union alternative
- **THEN** semantic analysis reports a source-located diagnostic naming the unsupported alternative before backend emission

#### Scenario: Inline union payload exceeds the configured limit
- **WHEN** a union's inline payload exceeds the configured 4096-byte ceiling and no safe fallback is selected
- **THEN** compilation reports a stable source-located representation-limit diagnostic before native code generation

### Requirement: Compatible intersections preserve one underlying value
The checker SHALL accept statically satisfiable intersections of compatible structural shapes and contract views, including a compatible nominal class view when at most one unrelated concrete class representation is required. Intersections SHALL preserve the original value identity and expose only members whose combined constraints are valid. Conflicting or unsatisfiable intersections SHALL be rejected during semantic analysis. No intersection operation SHALL require a duplicate runtime object solely to combine static member information.

#### Scenario: Multiple structural shapes combine
- **WHEN** one object satisfies multiple compatible structural shape constraints
- **THEN** the intersection exposes the proven combined member set and retains the object's identity

#### Scenario: Structural shape and contract view combine
- **WHEN** one object is checked as both a structural shape and a compatible contract
- **THEN** property projection and contract method calls use the same underlying object and the statically proven view

#### Scenario: Compatible class and contract view combine
- **WHEN** an instance of one nominal class satisfies a contract and is viewed through their intersection
- **THEN** class identity and contract dispatch remain unchanged and both proven member sets are available

#### Scenario: Conflicting intersection is rejected
- **WHEN** intersection members impose incompatible property types, method signatures, or nominal class constraints
- **THEN** semantic analysis reports the conflict before lowering and no runtime representation is emitted
