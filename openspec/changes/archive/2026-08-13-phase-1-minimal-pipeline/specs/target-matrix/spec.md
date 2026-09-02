## ADDED Requirements

### Requirement: The produced executable runs on the host

Verification SHALL check that the executable generated from Zirk code runs correctly on the host platform, not just that the object file is emitted.

Through Phase 0, object emission and a binary built directly from Rust were verified. With a real language ahead, what matters as evidence is that a `.zrk` file ends up as a running process.

#### Scenario: Reference program
- **WHEN** a `.zrk` program that prints a string is compiled and run
- **THEN** the process exits with exit code 0
- **AND** standard output contains exactly the expected string

#### Scenario: Verification on the supported platforms
- **WHEN** the continuous integration pipeline is run
- **THEN** compilation and execution of the reference program is verified on every platform in the matrix
