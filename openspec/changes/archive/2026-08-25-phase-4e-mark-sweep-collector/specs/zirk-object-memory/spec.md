## ADDED Requirements

### Requirement: The header carries the collector's bookkeeping

Every object with identity SHALL carry, in addition to the dispatch descriptor, the metadata the collector needs to enumerate and free unreachable objects — without any existing reader of the dispatch descriptor needing to change.

#### Scenario: The dispatch descriptor does not change
- **WHEN** an object's dispatch descriptor field is inspected after the header grows
- **THEN** its value and format are identical to those before the header grew

#### Scenario: Every allocation is enumerable
- **WHEN** the collector walks the set of allocated objects
- **THEN** it reaches every live or dead object exactly once, without walking memory not allocated by the runtime
