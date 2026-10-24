## ADDED Requirements

### Requirement: Dynamic object failures are catchable
Missing direct property reads, failed runtime contract conformance, and invalid JSON value conversion SHALL raise typed implicit runtime errors that are catchable through ordinary `try`/`catch`. These failures SHALL not require a declared `throws` clause and SHALL not abort the process. Presence-safe lookup SHALL avoid a missing-property error.

#### Scenario: Missing property error is caught
- **WHEN** direct access reads an absent object key inside `try` and catches the missing-property error
- **THEN** the matching catch branch runs

#### Scenario: Contract violation is caught
- **WHEN** runtime conformance finds a missing or incompatible required member
- **THEN** a catchable contract-violation error is raised

#### Scenario: JSON conversion error is caught
- **WHEN** JSON stringification encounters a non-JSON value or a cycle
- **THEN** a catchable JSON conversion error is raised and no partial string is returned
