## ADDED Requirements

### Requirement: Native Float128 execution preserves binary128 values on Windows
Native code generation and runtime linking SHALL preserve IEEE 754 binary128 operations and values on Windows x86_64 as on the other supported native platforms. Target-specific helper calls SHALL resolve to ABI-compatible implementations. Formatting and conversion SHALL operate on binary128 precision directly; generated code SHALL NOT route a value through `Float64` unless the source explicitly requests that conversion. Missing helper support SHALL fail with a target-specific diagnostic instead of producing a linker crash or an inaccurate executable.

#### Scenario: Windows binary128 helpers link and execute
- **WHEN** a Windows x86_64 program exercises arithmetic, comparison, conversion, and formatting on `Float128`
- **THEN** all required helper symbols resolve and the program returns the specified binary128 results

#### Scenario: Native formatting does not narrow
- **WHEN** native code formats a binary128 value not exactly representable as binary64
- **THEN** output preserves enough precision to round-trip the original binary128 value

### Requirement: Native collection storage follows generic Zirk layouts
Native code generation SHALL preserve every accepted `Map<K, V>` and `Set<T>` layout through storage, lookup, membership, mutation, iteration, algebra, and calls. Runtime entry descriptors SHALL retain scalar width, aggregate field offsets, nullability, enum and union tags, active-payload trace maps, and managed-reference identity. Package calls SHALL use compatible generic and composite metadata; incompatible layouts SHALL be rejected before emitting the call.

#### Scenario: Native map round-trips heterogeneous aggregate values
- **WHEN** a map stores and retrieves a tuple, payload enum, nullable union, or nested collection reference
- **THEN** the native result preserves all fields, active tags, null states, and managed identities

#### Scenario: Native set uses typed composite hash and equality
- **WHEN** a set stores a type with a valid derived or explicit `Hash` and `Equal` implementation
- **THEN** native membership, duplicate detection, and removal use those implementations and preserve insertion order

#### Scenario: Incompatible package layout is rejected
- **WHEN** a compiled package call uses generic collection or composite metadata incompatible with the callee
- **THEN** loading or linking rejects the mismatch before code execution
