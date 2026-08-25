## MODIFIED Requirements

### Requirement: Safety and concurrency grammar
The grammar SHALL parse unsafe function modifiers and blocks, `commit` regions, `task` blocks and callable sugar, `task scope`, `cancellation shield`, await timeouts, and `select` branches with `after`, `default`, and cancellation cases without introducing `async fn`.

#### Scenario: Select statement is parsed
- **WHEN** source contains task, channel, timer, and default select branches
- **THEN** the parser produces distinct guarded branches and their result bindings

### Requirement: Contextual safety restrictions
The parser and semantic frontend SHALL preserve enough contextual information to diagnose `await`, task/thread creation, or irreversible effects in a reversible unsafe transaction and unsafe-only operations outside an unsafe boundary.

#### Scenario: Commit appears outside unsafe
- **WHEN** source places `commit {}` outside an unsafe block
- **THEN** compilation fails with a contextual syntax or semantic diagnostic

## ADDED Requirements

### Requirement: `extern` declares a native function without a body

The grammar SHALL accept `extern "C" fn name(parameters): ReturnType;` as a top-level item — a function signature with no body, terminated by `;`. Only the `"C"` calling-convention literal SHALL be accepted; any other string literal in that position SHALL be rejected with a diagnostic naming the unsupported convention. An `extern` declaration SHALL NOT accept a body, a generic parameter list, or a decorator.

#### Scenario: Native declaration parses
- **WHEN** source contains `extern "C" fn strlen(s: Pointer<Byte>): UInt64;`
- **THEN** the parser produces a bodyless function item recording its `"C"` convention, parameters, and return type

#### Scenario: Unsupported convention is rejected
- **WHEN** source contains `extern "system" fn f(): Void;`
- **THEN** compilation fails identifying `"C"` as the only accepted convention

#### Scenario: A body is rejected
- **WHEN** source contains `extern "C" fn f(): Void { }`
- **THEN** compilation fails because an `extern` declaration cannot have a body
