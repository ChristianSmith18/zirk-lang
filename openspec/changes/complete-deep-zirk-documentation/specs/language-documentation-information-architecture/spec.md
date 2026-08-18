## ADDED Requirements

### Requirement: Deep documentation is delivered in three blocks
The handbook SHALL complete substantive documentation in this order: standard library with toolchain; testing with native and low-level development; tutorials with reference material. Each block SHALL be internally linked and audited before the next block is marked complete.

#### Scenario: First block is reviewed
- **WHEN** standard-library and toolchain tasks are marked complete
- **THEN** their module, API, command, pipeline, permission, error, performance, platform, example, status, and navigation coverage has been audited before testing work begins

### Requirement: Operational chapters support independent use
Every substantive module, tool, testing, or low-level chapter SHALL contain enough contracts, signatures or commands, examples, failures, permissions, blocking/cancellation, complexity/performance, platform behavior, implementation status, and source links for its subject, while omitting dimensions that genuinely do not apply.

#### Scenario: Reader opens a standard module
- **WHEN** a reader consults `std.fs`
- **THEN** the chapter is sufficient to select APIs, understand results and permissions, predict resource/cancellation behavior, and follow working examples without consulting historical notes

### Requirement: Tutorials compose documented contracts
Tutorials SHALL build complete progressively explained programs from APIs and semantics already documented in owning chapters, SHALL include expected output and failure handling, and SHALL link each major concept back to its reference owner.

#### Scenario: Reader completes an application tutorial
- **WHEN** the tutorial finishes
- **THEN** the reader has a coherent project layout, manifest, code, commands, expected behavior, tests, permissions, diagnostics guidance, and next references

### Requirement: Reference material is exhaustive and website-ready
Reference indexes SHALL cover the accepted grammar, keywords, operators, literals, attributes/decorators, diagnostics, CLI, configuration, permissions, standard library, feature status, compatibility, glossary, and normative sources with unique navigation entries and resolvable links.

#### Scenario: Website navigation is generated
- **WHEN** the future website consumes `SUMMARY.md` and reference pages
- **THEN** it can build a complete hierarchy and search surface without placeholder chapters or ambiguous duplicate owners

