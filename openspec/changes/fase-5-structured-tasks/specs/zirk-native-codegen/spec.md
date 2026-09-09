## MODIFIED Requirements

### Requirement: Codegen for cooperative task suspension

Native code generation SHALL emit, for each suspension point, a call into the
runtime that saves the running task's callee-saved registers, stack pointer, and
resume address into its task control block and transfers control to the executor,
plus a resume label the executor returns to. For a task-start instruction with
the `uncancelable` flag, codegen SHALL call the uncancelable-spawn runtime entry
point; for an `await` instruction with the `shielded` flag, codegen SHALL call
the shielded-await runtime entry point, which raises and lowers the running
task's shield depth around the wait. `Task<T>` and `Task.Final<T>` SHALL lower to
the same one-word handle representation. The generated calling convention for an
ordinary function SHALL be unchanged whether or not the function reaches a
suspension point internally.

#### Scenario: Suspension point emits a context save and a resume label

- **WHEN** codegen processes an `await` suspension point
- **THEN** the emitted code calls the runtime suspend routine and defines a resume label that the executor can jump back to

#### Scenario: Shielded await calls the shielded entry point

- **WHEN** codegen processes an `await` instruction whose `shielded` flag is set
- **THEN** the emitted code calls the shielded-await runtime entry point rather than the plain await entry point

#### Scenario: Uncancelable spawn calls the uncancelable entry point

- **WHEN** codegen processes a task-start instruction whose `uncancelable` flag is set
- **THEN** the emitted code calls the uncancelable-spawn runtime entry point and the resulting handle has the same representation as an ordinary task handle

#### Scenario: Function ABI is not split

- **WHEN** codegen emits two functions, one that awaits internally and one that does not
- **THEN** both are emitted with the same calling convention and are called identically
