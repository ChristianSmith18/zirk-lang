## ADDED Requirements

### Requirement: Float128 is verified across the supported target matrix
The target verification matrix SHALL compile `Float128` arithmetic, conversion, comparison, and formatting for every object-emission target defined by the compiler specification. Native integration SHALL run precision and boundary fixtures on each platform where the compiler workspace is natively built, including Windows x86_64. Target coverage SHALL report object-emission and native-run evidence separately; cross-target object emission SHALL NOT be reported as runtime verification.

#### Scenario: Float128 object emission covers every target
- **WHEN** the target-matrix workflow emits objects for all compiler-supported target triples
- **THEN** each target emits valid code for the documented `Float128` operation fixture or reports a target-specific blocking failure

#### Scenario: Float128 runs on Windows x86_64
- **WHEN** the native Windows x86_64 integration workflow compiles and runs the precision and boundary fixtures
- **THEN** linker resolution, execution, and output satisfy the binary128 contract

#### Scenario: Native evidence is reported distinctly
- **WHEN** the target matrix report summarizes Float128 coverage
- **THEN** it distinguishes cross-target object emission from executable runs on native CI platforms
