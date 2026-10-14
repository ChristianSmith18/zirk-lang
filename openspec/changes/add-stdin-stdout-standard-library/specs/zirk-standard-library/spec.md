## ADDED Requirements

### Requirement: std.io stream API has one normative owner

The standard-library inventory and handbook SHALL describe the delivered
`std.io` stdin, stdout, and stderr APIs using the signatures, `ReadResult`,
`IoError`, UTF-8, EOF, output ordering, and browser exclusions defined by the
standard-streams capability.

#### Scenario: Stream API changes
- **WHEN** a public `std.io` stream signature or behavior changes
- **THEN** the standard-streams specification, `ZIRK_STDLIB_SPEC.md`, handbook,
  examples, and status evidence are updated together
