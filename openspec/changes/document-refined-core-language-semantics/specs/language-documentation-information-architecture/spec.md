## ADDED Requirements

### Requirement: Completed core-language learning route
The handbook SHALL provide an ordered, linked route through callables, objects/contracts, generics, tuples/records/enums/unions, collections/iteration, and pattern matching, with conceptual pages before detailed APIs and reference tables after explanatory chapters.

#### Scenario: Agent follows implementation route
- **WHEN** an implementation agent starts from SUMMARY or the core-language overview
- **THEN** it can reach every normative owner, explanatory chapter, API/operator table, valid/invalid example, and implementation-status note without relying on planned placeholders

### Requirement: Obsolete placeholders are resolved
Unlinked planned entries that duplicate published units SHALL be removed or converted into intentional links, while genuinely future areas SHALL remain clearly identified as planned rather than appearing complete.

#### Scenario: Duplicate type-model placeholder
- **WHEN** the type model already has a published canonical unit
- **THEN** SUMMARY does not retain a second unlinked placeholder for the same material
