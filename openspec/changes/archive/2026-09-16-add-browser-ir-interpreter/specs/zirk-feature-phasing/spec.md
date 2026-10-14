## ADDED Requirements

### Requirement: Browser interpretation is distinct from a WebAssembly language target

Project phase and implementation-status documentation SHALL describe browser execution as compiler tooling in which the Rust frontend and IR interpreter are compiled to WebAssembly. It SHALL NOT claim that Zirk programs compile to WebAssembly or that `wasm32` is a supported Zirk target.

#### Scenario: Publish browser playground availability
- **WHEN** the browser interpreter and companion playground integration are complete
- **THEN** status documentation may state that supported Zirk programs can be interpreted locally in the browser
- **AND** it continues to list a Zirk-to-WebAssembly backend as excluded from Zirk 1.x or deferred to its future phase

#### Scenario: Inspect target support
- **WHEN** a user reads the target matrix after this change
- **THEN** no WebAssembly target triple is listed as a supported Zirk compilation target

### Requirement: Browser compatibility claims remain scoped

Implementation-status and limitation documentation SHALL distinguish the native compiler's delivered language surface from the browser interpreter's compatibility surface. A feature SHALL NOT be marked browser-executable merely because the parser, checker, IR, native backend, or native runtime implements it.

#### Scenario: Native-only feature is implemented
- **WHEN** a language feature works through native compilation but requires an excluded browser capability
- **THEN** browser compatibility documentation identifies it as unavailable or constrained

#### Scenario: Browser-compatible feature is claimed
- **WHEN** a feature is described as executable in the browser
- **THEN** interpreter coverage, browser-package tests, and differential conformance evidence exist for that feature

### Requirement: Cross-repository status is released coherently

Public browser-execution status SHALL be updated in the compiler repository and `zirk-lang-site` within the same coordinated workstream. The website SHALL pin the exact clean compiler revision and browser artifact provenance that support its claim.

#### Scenario: Compiler support lands before site integration
- **WHEN** interpreter implementation exists but the website has not consumed and verified its package
- **THEN** the public site continues to describe Run and Check as unavailable

#### Scenario: Publish the integrated feature
- **WHEN** compiler conformance, artifact provenance, site integration, accessibility, and Firebase build verification are complete
- **THEN** both repositories describe the same supported browser execution surface and exclusions
