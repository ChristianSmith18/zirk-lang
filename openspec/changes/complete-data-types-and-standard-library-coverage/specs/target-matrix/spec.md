## ADDED Requirements

### Requirement: Advanced type features have stage-specific target evidence

The support matrix for generic enum dispatch, admitted union/intersection combinations, aggregate `Hash`/`Equal`, and enum `Clone` SHALL report separate results for parsing/type checking, lowering, interpreter execution, object emission, native linking, and native execution. Native execution SHALL be verified on every supported CI host: Linux x86_64 and aarch64, macOS x86_64 and aarch64, and Windows x86_64. Object emission SHALL cover all nine compiler targets already defined by this capability. Evidence for one host or one pipeline stage SHALL NOT imply another host or stage.

#### Scenario: Native matrix runs the advanced feature corpus

- **WHEN** the supported native CI matrix runs the advanced type corpus
- **THEN** each host records the compiler/runtime revision and passes its executable behavior cases or is reported as an explicit failure

#### Scenario: Object-emission matrix includes generic enums

- **WHEN** the nine-target object-emission matrix compiles the generic enum and algebraic payload corpus
- **THEN** each target emits a valid object or reports a target-specific unsupported capability before emitting invalid output

#### Scenario: Evidence labels distinguish runtime from emission

- **WHEN** a target has object-emission evidence but no native host execution
- **THEN** its published status says object emission only and does not claim runtime execution

### Requirement: Float128 target availability is explicit and verified

`Float128` SHALL either be supported through type checking, lowering, runtime operations, and executable behavior on Windows x86_64, or the source contract and feature status SHALL declare its precise Windows exclusion and reject unsupported use with a source diagnostic. Object emission alone SHALL NOT establish Windows runtime support.

#### Scenario: Float128 Windows behavior is supported

- **WHEN** the Windows x86_64 native CI target compiles and runs the Float128 boundary and arithmetic corpus
- **THEN** results match the language contract and the status catalog records native execution evidence

#### Scenario: Float128 Windows use is excluded

- **WHEN** Float128 is outside the declared Windows contract
- **THEN** a source-located diagnostic rejects each unsupported use before backend emission and the status catalog names the exact exclusion
