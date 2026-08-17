## Purpose

Define the teaching, source-fidelity, navigation, review, and language standards for the public English Zirk handbook.
## Requirements
### Requirement: English public handbook
The handbook SHALL present all public-facing prose, headings, navigation labels, example commentary, and diagnostics explanations in English.

#### Scenario: Public reader opens a chapter
- **WHEN** a reader opens any document under `docs/handbook/`
- **THEN** the instructional content and navigation are written in English

### Requirement: Flexible problem-led chapters
Each substantive chapter SHALL introduce the reader's problem or question before formal detail, SHALL teach terminology in context, and SHALL include only the explanatory sections appropriate to that topic rather than conforming to a fixed document length.

#### Scenario: Author documents a complex semantic feature
- **WHEN** the feature requires syntax, inference, constraints, diagnostics, interactions, and rationale to be understood safely
- **THEN** the chapter includes those sections and enough examples to explain them

#### Scenario: Author documents a short orientation topic
- **WHEN** the topic can be explained completely without every optional section
- **THEN** the chapter remains focused and does not add filler merely to match other chapters

### Requirement: Authentic code-based teaching
Language chapters MUST use specification-compatible Zirk code and SHALL include complete valid examples, invalid examples with expected diagnostics where mistakes are instructive, and corrected versions where applicable.

#### Scenario: Reader learns a constrained feature
- **WHEN** a chapter explains a feature with compile-time or runtime restrictions
- **THEN** the chapter shows a valid use and at least one representative failure with an explanation of the correction

### Requirement: Normative and implementation status disclosure
Each substantive chapter SHALL identify its normative source, and SHALL distinguish target language behavior from currently implemented compiler behavior whenever they differ.

#### Scenario: Specified feature is not fully implemented
- **WHEN** a chapter teaches behavior defined by a final specification but absent from the current compiler milestone
- **THEN** the chapter displays a visible implementation-status note without weakening the normative description

### Requirement: Bidirectional ordered navigation
Every document that participates in a sequential handbook path SHALL end with explicit links to the previous and next documents, using a beginning or end label when one side has no adjacent document.

#### Scenario: Reader finishes an ordered chapter
- **WHEN** the reader reaches the chapter footer
- **THEN** the reader can move to the preceding or following chapter without returning to the table of contents

### Requirement: Canonical handbook order
The handbook MUST provide a root `SUMMARY.md` that names every published chapter and defines its canonical sequence and hierarchy.

#### Scenario: Navigation is audited
- **WHEN** the handbook tree and chapter footers are compared with `SUMMARY.md`
- **THEN** every published ordered chapter appears once and its previous and next links match the canonical sequence

### Requirement: Editorial completion criteria
A chapter MUST NOT be marked complete unless its links resolve, its examples have been checked against the cited specifications, its terminology is consistent with adjacent chapters, and its depth is sufficient for its declared audience.

#### Scenario: Chapter review completes
- **WHEN** an editor marks a handbook task complete
- **THEN** the chapter satisfies link, source-fidelity, terminology, example, and audience checks

### Requirement: Authorial review corrections
The handbook MUST incorporate every numbered clarification in the language author's annotated review and MUST propagate each clarification to all other handbook pages that describe the same syntax or semantics.

#### Scenario: A clarification affects multiple chapters
- **WHEN** an annotation changes a rule documented in a tutorial, language chapter, and reference page
- **THEN** all affected pages present the same rule and compatible examples

### Requirement: In-depth per-type chapter contract
Every built-in type chapter SHALL explain purpose, construction/literals, inference, storage category, mutability, conversions, native operators, properties, methods, controlled errors, valid examples, invalid examples, and interactions when applicable. Chapter length SHALL follow semantic complexity rather than a fixed template size.

#### Scenario: Complex type receives deeper treatment
- **WHEN** `String`, `Float`, `ZonedDateTime`, or `Duration` has edge cases absent from `Boolean`
- **THEN** its chapter includes the additional explanations and examples instead of being constrained to the same size

### Requirement: Operator and API tables remain explanatory
Operator matrices and property/method catalogs SHALL state operand types, result types, mutation behavior, error conditions, and link to explanatory examples. A table SHALL NOT be the sole explanation of surprising behavior.

#### Scenario: Contextual cast reference entry
- **WHEN** the reference table lists `Float(a / b)`
- **THEN** it links to prose that explains operand conversion before evaluation and contrasts ordinary integer division

### Requirement: Type documentation contradiction audit
Completion SHALL include repository-wide checks for obsolete `Decimal*` naming, code-point-only `Char`, immutable or copy-on-write String claims, non-signed Duration claims, permissive strict aliases, and incompatible native operator examples.

#### Scenario: Superseded claim remains
- **WHEN** an audit finds a published page calling String immutable
- **THEN** the change remains incomplete until the page is corrected or explicitly scoped to a different value

### Requirement: Cross-cutting core semantics remain synchronized
Every accepted callable, projection, object, generic, algebraic-data, collection, iteration, and matching rule SHALL be updated consistently in normative documents, owning handbook chapters, reference indexes, examples, source maps, and affected active planning artifacts. Historical archives SHALL remain historical.

#### Scenario: Projection rule audit
- **WHEN** projection-copy documentation is completed
- **THEN** repository-wide checks find no current claim that nested reference extraction aliases its container

### Requirement: Final semantics and implementation status are separate
Documentation SHALL state final language semantics independently from the compiler phase that delivers them and SHALL link undelivered features to status/roadmap material without weakening or contradicting the final rule.

#### Scenario: Escaping closures before implementation
- **WHEN** readers inspect Fn before its compiler phase ships
- **THEN** they see both the final legal behavior and an explicit implementation-status notice

### Requirement: Failure, resource, and permission chapters teach complete contracts
The handbook SHALL document the accepted model with mental models, syntax, valid and invalid examples, API/exception/permission tables, composition behavior, diagnostics, security rationale, implementation status, and previous/next links. It SHALL distinguish final semantics from historical syntax and current compiler availability.

#### Scenario: Contributor reads error unit
- **WHEN** a contributor follows the error handbook unit
- **THEN** they can determine whether a failure uses `Result`, declared `throws`, implicit `RuntimeError`, or `fatalError`, and how it composes with cleanup

### Requirement: Security-sensitive prompts and policies have worked examples
Permission documentation SHALL show manifest requests/grants, dependency paths, moved-project reapproval, update reapproval, incremental fast paths, broad-grant confirmation, CI policy, runtime denial, audit history, and tamper scenarios.

#### Scenario: Malicious manifest edit is explained
- **WHEN** a reader examines the permission-security chapter
- **THEN** it explicitly demonstrates that changing `init.zrk` cannot create a valid signed approval

### Requirement: Safety and concurrency source synchronization
The editorial system SHALL identify canonical owners for memory/unsafe and concurrency semantics and SHALL update all derivative handbook, reference, roadmap, example, and agent-context pages when those rules change.

#### Scenario: Unsafe rollback rule changes
- **WHEN** the canonical transactional unsafe rule is edited
- **THEN** pointer, unsafe, runtime, compiler, example, and reference pages are checked for contradictory wording
