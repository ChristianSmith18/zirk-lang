## RENAMED Requirements

- FROM: `### Requirement: Executor owns the root scope of \`main\``
- TO: `### Requirement: Executor owns the root scope of the selected entry`

## MODIFIED Requirements

### Requirement: Executor owns the root scope of the selected entry

The runtime SHALL run the selected Zirk entry file's top-level body as the root branch of an implicit `concurrent` scope and SHALL NOT return the program's exit status until that branch and every branch it owns have completed, or been cancelled and cleaned (ambient timers are cancelled on normal exit). An exception that escapes the root branch SHALL cause a nonzero exit status.

#### Scenario: Program waits for spawned children
- **WHEN** the selected entry spawns a branch and reaches the end of its body while the branch runs
- **THEN** the process does not exit until the branch has finished or been cancelled and cleaned

#### Scenario: Uncaught failure in the root branch
- **WHEN** an exception escapes the selected entry body after children are cleaned
- **THEN** the process exits with a nonzero status
