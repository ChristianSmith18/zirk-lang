## ADDED Requirements

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
