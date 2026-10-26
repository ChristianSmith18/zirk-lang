## ADDED Requirements

### Requirement: Composite types lower to verified portable IR
Every source union accepted by semantic analysis SHALL have a typed portable-IR representation and explicit checked injection, tag inspection, payload projection, and control-flow dispatch. Intersections SHALL retain all static member information while reusing the underlying value representation whenever safe. Singleton types and compile-time type operators SHALL not add runtime metadata except where a union representation requires a tag. Unsupported representations SHALL fail in lowering with a source diagnostic rather than reaching a backend panic.

#### Scenario: Heterogeneous union argument is lowered
- **WHEN** a function accepts String | Int32
- **THEN** the call site injects the concrete value and the callee receives a verified union-typed IR value

#### Scenario: Union match projects the selected payload
- **WHEN** a match dispatches on a tagged union alternative
- **THEN** each arm receives a value with the alternative's exact IR type

#### Scenario: Intersection is a static view
- **WHEN** one object is passed through an intersection of compatible contracts
- **THEN** IR preserves the same object identity and exposes the combined checked operations without a duplicate object

#### Scenario: Managed payload metadata survives union storage
- **WHEN** a union alternative contains a managed reference or nested managed value
- **THEN** IR retains sufficient tag and layout information for safe tracing and copying

#### Scenario: Invalid union IR is rejected before codegen
- **WHEN** a malformed union tag or payload operation is constructed
- **THEN** IR verification rejects it and no invalid module reaches LLVM emission
