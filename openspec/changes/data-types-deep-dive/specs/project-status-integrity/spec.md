# Project Status Integrity — Data Types Deep Dive

## ADDED Requirements

### Requirement: New type chapters stay consistent with the feature-status catalog

Any expanded or new type chapter that makes an implementation-status claim (e.g., "`List<T>` is delivered" or "`Regex` is specified") SHALL reference `docs/init/ZIRK_FEATURE_STATUS.md` and SHALL be updated in the same change if the status catalog is updated. The chapter SHALL NOT promote a feature to implemented unless the catalog already marks it implemented.

#### Scenario: A chapter mentions a delivered collection

- **WHEN** the `List` chapter states that `List<T>` is implemented
- **THEN** the feature-status catalog contains the same status and the change includes both updates

#### Scenario: A chapter describes a specified but pending feature

- **WHEN** the `Regex` or `Map` chapter documents behavior not yet implemented
- **THEN** the page displays a visible implementation-status notice and does not claim the feature runs today
