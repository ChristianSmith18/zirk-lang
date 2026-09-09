## ADDED Requirements

### Requirement: Deterministic object spread lowering

IR lowering SHALL evaluate object spread sources and explicit fields left to right, copy fields according to nominal layout metadata, apply later explicit overrides, and construct the destination without mutating any source object.

#### Scenario: Object override order
- **WHEN** `{ ...base, name: replacement }` is lowered
- **THEN** `base` is evaluated first and `replacement` becomes the final `name` field
