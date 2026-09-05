# zirk-data-types — Documentation Examples for the Deep Dive

## ADDED Requirements

### Requirement: Example-driven documentation without semantic change

The `zirk-data-types` specification examples in the handbook SHALL illustrate the value/reference/unsafe spectrum for tuples, records, enums, unions, aliases, and built-in types, but SHALL NOT introduce new language semantics, new keywords, or unapproved type syntax.

#### Scenario: A record example shows value semantics

- **WHEN** the `record` chapter is expanded
- **THEN** it includes a code example that demonstrates independent copies after assignment and explains why `is` is invalid for records

#### Scenario: A tuple example shows compile-time index

- **WHEN** the `tuple` chapter is expanded
- **THEN** it demonstrates `tuple.0` and `tuple[-1]` as valid and `tuple[i]` with a runtime index as an error

### Requirement: Invalid examples match the compiler

Every invalid example in the new or expanded type chapters SHALL cite a diagnostic or controlled error that the current compiler or the normative spec defines, and SHALL NOT invent new error names.

#### Scenario: List mutation through an inmut::strict alias

- **WHEN** the `List` chapter shows an invalid example
- **THEN** the example names the expected borrow/alias error and, if the feature is not yet implemented, marks it with an implementation-status notice
