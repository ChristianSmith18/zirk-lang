## ADDED Requirements

### Requirement: Chapter depth is contract-based
A substantive chapter SHALL be complete only when it answers the reader's likely operational questions and covers every applicable dimension of its topic; line count SHALL NOT be used as a completion criterion and short index pages SHALL NOT be padded.

#### Scenario: Short substantive page is reviewed
- **WHEN** a module page contains only a summary and navigation
- **THEN** it remains incomplete until applicable APIs, semantics, examples, errors and constraints are documented

### Requirement: Examples are executable or explicitly scoped
Every code or command example SHALL be checked against normative syntax and contracts, SHALL identify required imports/configuration/permissions when material, and SHALL be labeled when it illustrates target semantics unavailable in the current compiler.

#### Scenario: Aspirational example is published
- **WHEN** an example uses a specified but unimplemented feature
- **THEN** the surrounding chapter shows a visible status note and does not claim successful execution on the current compiler

### Requirement: Block completion includes editorial audits
Each documentation block SHALL verify local links, SUMMARY membership, previous/next navigation, code fences, terminology, canonical-source fidelity, contradictions, and implementation-status claims before its tasks are completed.

#### Scenario: Block contains a broken adjacent link
- **WHEN** the block audit finds the link
- **THEN** the block remains incomplete until the link is corrected

### Requirement: Active main specs have meaningful purposes
Every main capability spec SHALL state a concise purpose describing the behavior it governs and SHALL NOT retain archive-generated `Purpose: TBD` text.

#### Scenario: Archived capability has placeholder purpose
- **WHEN** the final documentation audit encounters `TBD - created by archiving change`
- **THEN** it replaces the placeholder with a capability-specific purpose before declaring website readiness

