## MODIFIED Requirements

### Requirement: Codegen for cooperative task suspension

Native code generation SHALL emit, for each suspension point, a call into the runtime that saves the running branch's callee-saved registers, stack pointer, and resume address into its control block and transfers control to the executor, plus a resume label the executor returns to. The generated calling convention for an ordinary function SHALL be unchanged whether or not the function reaches a suspension point internally. Suspension points are `Timer.sleep`, a blocking channel operation, and an explicit cancellation check; the specific instructions that emit them are defined by `concurrent-blocks-and-timers` and `typed-channels`.

#### Scenario: Suspension point emits a context save and a resume label

- **WHEN** codegen processes a suspension point
- **THEN** the emitted code calls the runtime suspend routine and defines a resume label that the executor can jump back to

#### Scenario: Function ABI is not split

- **WHEN** codegen emits two functions, one that suspends internally and one that does not
- **THEN** both are emitted with the same calling convention and are called identically
