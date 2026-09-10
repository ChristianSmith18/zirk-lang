## MODIFIED Requirements

### Requirement: Codegen for cooperative task suspension

Native code generation SHALL emit, for each suspension point (`Timer.sleep`, a
blocking channel operation, `JobWait`, an explicit cancellation check), a call
into the runtime that saves the running branch's callee-saved registers, stack
pointer, and resume address into its control block and transfers control to the
executor, plus a resume label the executor returns to. `BranchStart` SHALL emit a
per-site `extern "C" fn(ptr) -> i64` thunk that reloads captured values, calls the
lifted branch body, and widens the result to the runtime's branch-result word;
codegen SHALL call `zirk_rt_spawn(thunk, capture_block)` for the handle and
`zirk_rt_job_wait(job)` for `JobWait`. `IrType::Job` SHALL lower to LLVM `i64`.
The generated calling convention for an ordinary function SHALL be unchanged
whether or not it reaches a suspension point internally.

#### Scenario: Suspension point emits a context save and a resume label
- **WHEN** codegen processes a `JobWait` or `Timer.sleep` suspension point
- **THEN** the emitted code calls the runtime suspend routine and defines a resume label

#### Scenario: BranchStart emits a thunk and a spawn call
- **WHEN** codegen processes a `BranchStart`
- **THEN** it emits the capture block, a branch thunk, and a `zirk_rt_spawn` call yielding the `i64` job handle

#### Scenario: Function ABI is not split
- **WHEN** codegen emits two functions, one that suspends internally and one that does not
- **THEN** both are emitted with the same calling convention and are called identically
