## RENAMED Requirements

- FROM: `### Requirement: Lifecycle of the generated program`
- TO: `### Requirement: Native entrypoint lifecycle`

## MODIFIED Requirements

### Requirement: Native entrypoint lifecycle

Native code generation SHALL emit an internal platform entrypoint that initializes the runtime, initializes application globals, executes the statically selected Zirk entry body after its imported modules, and performs structured shutdown. The process entrypoint SHALL return the runtime's process status only after the application body and all descendant tasks/resources have completed or been cleaned. No source-level function named `main` SHALL be required or invoked implicitly.

#### Scenario: Native startup executes the selected file body
- **WHEN** a compiled application starts with a manifest-selected entry
- **THEN** the internal platform entrypoint runs that Zirk file's top-level body after imported module initialization

#### Scenario: Startup runs the executor loop
- **WHEN** a compiled program starts
- **THEN** the internal platform entrypoint runs the selected Zirk body as the executor's root task and waits for descendants to finish or clean up

#### Scenario: Runtime lifecycle remains internal
- **WHEN** a Zirk source file has no `fn main`
- **THEN** the generated executable initializes and shuts down the runtime without requiring a user-defined entry function

#### Scenario: Descendant task is still cleaning up
- **WHEN** the application entry body completes while a descendant task is still running or cleaning up
- **THEN** the process waits for the executor lifecycle to finish before returning its status

#### Scenario: Invocation order
- **WHEN** the generated entrypoint is inspected
- **THEN** it initializes the runtime and globals before the selected file body
- **AND** it shuts down the runtime after the body and its structured work complete

#### Scenario: Exit code
- **WHEN** a program terminates normally
- **THEN** the process returns the runtime's resulting status

### Requirement: Executor lifecycle wraps the entrypoint

Native code generation SHALL emit program startup that runs the selected Zirk
file body as the executor's root task and returns the process exit status only
after that task and all descendants have completed or been cleaned.

#### Scenario: Startup runs the executor loop
- **WHEN** a compiled program starts
- **THEN** the selected file body runs as the root task and the process does not exit while a descendant task is still running or cleaning up
