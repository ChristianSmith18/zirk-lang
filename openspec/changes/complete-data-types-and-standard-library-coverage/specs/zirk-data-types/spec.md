## MODIFIED Requirements

### Requirement: Enums retain component-wise cloning

An enum SHALL implement `Clone` if and only if every associated payload type implements `Clone`; derivation SHALL recurse through supported tuples, nullable values, nested enums, unions, and cloneable class references using the ordinary graph-clone guarantees. Cloned payloads SHALL follow each component's `Clone` semantics, including preservation of internal sharing and cycles for reference graphs. A dynamic `Object`, resource, pointer, lock, task, or other non-`Clone` payload SHALL NOT receive an implicit clone implementation. Unsupported derivation SHALL be rejected at the declaration or use site with the exact payload path identified.

#### Scenario: Clone nested cloneable enum payloads

- **WHEN** an enum payload contains nested tuples, nullable values, another enum, or a cloneable class graph and every component implements `Clone`
- **THEN** the enum clone succeeds and each payload follows its declared component clone semantics

#### Scenario: Clone an enum with cloneable payloads

- **WHEN** every associated type of an enum implements `Clone` and an enum value is cloned
- **THEN** the clone contains independently cloned payload values

#### Scenario: Reject a non-Clone payload path

- **WHEN** an enum payload recursively contains a resource or other non-`Clone` value
- **THEN** `Clone` derivation is rejected and the diagnostic identifies the variant and nested payload path

## ADDED Requirements

### Requirement: Hash and Equal derivation follows stable value semantics

The language SHALL derive `Hash` and `Equal` for supported immutable value compositions when every component supplies compatible capabilities. Derived equality and hashing SHALL be recursive, deterministic for the lifetime of a stored key, and satisfy `Equal(a, b) => Hash(a) = Hash(b)`. Mutable reference objects SHALL use stable identity or an explicit user-provided capability; they SHALL NOT be recursively hashed by mutable contents by default. String and Char components SHALL use the language's content equality, and text components stored in hash collections SHALL use the immutable insertion snapshot required by collection semantics.

Collection equality SHALL form a total equivalence relation even where language `==` follows partial IEEE comparison. Floating collection keys/elements SHALL normalize signed zero and treat every NaN encoding of the same width as one value; infinities retain their sign. Decimal equality SHALL use exact numeric value after canonical coefficient/scale normalization. Nullable values SHALL include presence in equality and hashing. Tuple fields compare in declaration order; enum and union alternatives compare their canonical active tag before recursively comparing payload fields. Immutable structural derivation is restricted to declarations with immutable value semantics.

#### Scenario: Derived tuple uses component equality

- **WHEN** a tuple contains scalar, String, Char, nullable Decimal, or another supported immutable value component
- **THEN** derived equality compares each component by its language semantics and derived hashing uses the same component boundaries

#### Scenario: Mutable object is not implicitly structurally hashed

- **WHEN** a mutable `Object` is used as a structural hash component without stable identity or an explicit supported immutable representation
- **THEN** the compiler rejects derivation instead of allowing mutations to invalidate a hash collection

### Requirement: Algebraic values preserve layout and behavior across lowering and runtime

Every union and intersection composition admitted by semantic analysis SHALL have defined portable-IR, interpreter, and native representations for its active alternative, payload, narrowing, matching, error paths, cleanup, and managed references. Intersections SHALL remain views over one underlying value; unions SHALL retain the canonical active alternative whenever matching, dispatch, or projection depends on it. A composition outside the supported layout matrix SHALL be rejected before backend emission. The 4096-byte inline payload ceiling SHALL remain an explicit limit; this requirement SHALL NOT imply unbounded inline payload storage.

#### Scenario: Lower and match a nested union in a container

- **WHEN** a supported union containing a supported composite payload is stored in or retrieved from a managed collection and narrowed with `match`
- **THEN** interpreter and native execution select the same active alternative, preserve the payload, propagate errors, and run cleanup exactly once

#### Scenario: Unsupported alternative family is diagnosed early

- **WHEN** a union or intersection composition exceeds the declared layout matrix or inline payload ceiling
- **THEN** compilation reports the unsupported type composition before IR or backend emission and does not panic
