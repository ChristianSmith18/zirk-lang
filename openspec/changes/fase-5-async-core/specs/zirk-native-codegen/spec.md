## ADDED Requirements

### Requirement: Codegen for cooperative task suspension

Native code generation SHALL emit, for each suspension point, a call into the
runtime that saves the running task's callee-saved registers, stack pointer, and
resume address into its task control block and transfers control to the executor,
plus a resume label the executor returns to. The generated calling convention for
an ordinary function SHALL be unchanged whether or not the function reaches a
suspension point internally.

#### Scenario: Suspension point emits a context save and a resume label

- **WHEN** codegen processes an `await` suspension point
- **THEN** the emitted code calls the runtime suspend routine and defines a resume label that the executor can jump back to

#### Scenario: Function ABI is not split

- **WHEN** codegen emits two functions, one that awaits internally and one that does not
- **THEN** both are emitted with the same calling convention and are called identically

### Requirement: Context-switch support for every supported target

The runtime staticlib SHALL provide a context-switch implementation for each
target triple the toolchain supports (`ADR-004`): macOS aarch64, Linux x86_64,
Linux aarch64, and Windows x86_64. The implementation SHALL correctly save and
restore the callee-saved register set and stack pointer for that platform's
calling convention and SHALL switch to a task-owned stack.

#### Scenario: Round-trip context switch preserves state

- **WHEN** the runtime switches from the executor to a task and back on a supported target
- **THEN** all callee-saved registers and the stack pointer hold their pre-switch values on return

#### Scenario: Every supported target has an implementation

- **WHEN** the runtime is built for any supported target triple
- **THEN** a context-switch implementation for that triple is compiled in

### Requirement: Per-task shadow-stack frame registration

Native code generation SHALL emit shadow-stack frame push and pop operations that
target the running task's control block rather than a single process-global
chain. The runtime SHALL track the current task so that push and pop reach the
correct chain, and the collector SHALL walk every live task's chain during root
enumeration.

#### Scenario: Frame push reaches the running task's chain

- **WHEN** a function activation on a task's stack pushes its shadow-stack frame
- **THEN** the frame is linked into that task's chain, not another task's

#### Scenario: Collection walks all task chains

- **WHEN** a collection runs while several tasks are suspended
- **THEN** root enumeration visits the shadow-stack chain of every live task

### Requirement: Executor lifecycle wraps the entrypoint

Native code generation SHALL emit program startup that runs `main`'s body as the
executor's root task and returns the process exit status only after that task and
all descendants have completed or been cleaned.

#### Scenario: Startup runs the executor loop

- **WHEN** a compiled program starts
- **THEN** `main`'s body runs as the root task and the process does not exit while a descendant task is still running or cleaning up
