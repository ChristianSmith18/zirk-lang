## ADDED Requirements

### Requirement: Collection elements retain nested managed references

`Array<T>` and `List<T>` SHALL preserve every managed reference nested in a supported `T`, including references in tuples, records, enums, nullable values, and closures. Growth, slicing, cloning, and element replacement SHALL preserve valid tracing metadata.

#### Scenario: Composite array element is the only owner
- **WHEN** an array is the only reachable owner of an object nested in one of its composite elements and collection runs
- **THEN** the object remains valid while that array element remains reachable

#### Scenario: Composite list is resized and collected
- **WHEN** a list containing composite reference-bearing elements grows or shrinks and collection runs
- **THEN** all live element references remain valid and removed-element references are no longer retained by stale metadata

### Requirement: Collection representation is checked before code generation

Every collection element type accepted by semantic analysis SHALL have defined storage, equality or hashing where applicable, ABI conversion, and GC tracing behavior. Unsupported combinations SHALL fail in semantic analysis with a diagnostic naming the type and operation.

#### Scenario: Unsupported map value is rejected early
- **WHEN** a map operation uses a value representation that has no runtime encoding
- **THEN** semantic analysis reports the unsupported representation before IR or LLVM emission

#### Scenario: Supported composite element is lowered
- **WHEN** a supported composite element type is used in an array or list
- **THEN** lowering and code generation preserve its layout and managed-reference paths
