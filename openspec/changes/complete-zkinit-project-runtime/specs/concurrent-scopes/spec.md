## RENAMED Requirements

- FROM: `### Requirement: \`main\` runs in an implicit \`concurrent\` scope`
- TO: `### Requirement: The selected entry runs in an implicit \`concurrent\` scope`

## MODIFIED Requirements

### Requirement: The selected entry runs in an implicit `concurrent` scope

The selected application entry's top-level body SHALL be an implicit `concurrent` scope. Top-level `spawn`, `Timer.after`, and `Timer.every` SHALL be owned by it. The process SHALL NOT exit until that body has completed and its owned branches have been joined or (for ambient timers, and on an exceptional exit) cancelled and cleaned.

#### Scenario: spawned work outlives the selected entry body
- **WHEN** the selected entry spawns a branch and then reaches the end of its body
- **THEN** the implicit scope waits for that branch before the process exits
