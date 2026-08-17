## ADDED Requirements

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
