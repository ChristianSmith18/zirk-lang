## MODIFIED Requirements

### Requirement: Native code generation preserves union value semantics
Native code generation SHALL represent every accepted heterogeneous union, including structural alternatives and mixed structural/nominal/scalar alternatives, with a target-correct tag and payload layout. It SHALL preserve alternative identity through parameters, returns, local and global storage, captures, aggregate values, and compatible package calls. Tag dispatch and payload projection SHALL not expose raw representation to safe source code. Managed-reference metadata and root enumeration SHALL describe only the active payload and remain correct through copying, collection, and control-flow joins. Inline storage SHALL be used only where its measured layout and target ABI are safe; a managed or boxed fallback SHALL be used where necessary without changing source semantics. Package boundaries SHALL agree on canonical alternatives, representation metadata, and calling convention, or reject incompatible artifacts before execution. Intersections SHALL remain static views and SHALL NOT allocate a second runtime object solely to combine member information.

#### Scenario: Union of distinct scalar representations
- **WHEN** a native function accepts and returns `String | Int32`
- **THEN** both alternatives round-trip correctly without type confusion

#### Scenario: Union with a managed object alternative
- **WHEN** a union contains a scalar alternative and a managed-reference alternative
- **THEN** tag dispatch and root enumeration identify the active representation correctly

#### Scenario: Structural union crosses calls and storage
- **WHEN** a tagged structural value is passed, returned, stored, captured, and narrowed
- **THEN** its selected alternative and property values remain correct at every boundary

#### Scenario: Mixed structural and nominal alternatives
- **WHEN** a union contains structural shapes together with class, enum, or scalar alternatives
- **THEN** native layout, payload projection, and narrowing preserve each alternative without confusing references and inline values

#### Scenario: Managed nested payload is traced
- **WHEN** an active structural payload contains one or more managed references
- **THEN** generated root metadata traces the active references and does not interpret inactive payload bytes as roots

#### Scenario: Union remains internal to a package ABI
- **WHEN** a compiled package exposes a union-typed callable to another compatible package
- **THEN** both sides agree on canonical alternatives, tag meaning, payload layout, and calling convention

#### Scenario: Incompatible package representation is rejected
- **WHEN** a package caller and callee were compiled with incompatible union representation metadata
- **THEN** loading or linking rejects the mismatch before the call executes

#### Scenario: Target ABI differs
- **WHEN** two supported native targets classify a union aggregate differently
- **THEN** generated code uses each target's correct internal ABI while preserving the same Zirk semantics

#### Scenario: Oversized inline payload uses a safe fallback
- **WHEN** a structural union payload cannot safely use the target's inline representation
- **THEN** generated code uses a managed fallback or emits a source diagnostic, never an invalid LLVM aggregate

#### Scenario: Intersection view preserves identity
- **WHEN** one object is passed through an intersection of compatible contracts or structural types
- **THEN** native execution uses the same object identity and exposes the combined checked operations

## ADDED Requirements

### Requirement: Generic contract methods preserve specialized dispatch
Generic contract methods SHALL dispatch to an implementation compatible with the receiver's contract arguments and the call site's inferred method arguments. The compiler SHALL specialize method implementations for concrete compile-time substitutions or use an equivalent statically verified mechanism; it SHALL NOT require a runtime generic type descriptor. Exported package metadata SHALL fingerprint method type parameters, constraints, template patterns, and substituted callable signatures so incompatible callers are rejected before code emission or execution.

#### Scenario: Dynamic generic method dispatch uses the inferred specialization
- **WHEN** a `Watched<Person>` receiver is invoked with `"ageChanged"` and the call infers `K = "age"`
- **THEN** native dispatch reaches an implementation compatible with the `Person["age"]` callback signature and passes no runtime value for `K`

#### Scenario: Stale generic contract metadata is rejected
- **WHEN** a package caller was compiled against a different generic contract-method constraint or open-template pattern
- **THEN** package validation rejects the mismatch before emitting a native call site
