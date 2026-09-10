## REMOVED Requirements

### Requirement: Task creation and scope syntax
**Reason**: `task` / `task { }` / `task scope` are removed from the language.
**Migration**: `concurrent { }` and `spawn`, defined by `concurrent-blocks-and-timers`.

### Requirement: Await and timeout syntax
**Reason**: `await` and `await ... timeout` are removed.
**Migration**: a `concurrent` block joins its branches at the closing brace; `job.wait()` collects a `Job<T>`; `Concurrent.of(fn).within(d)` is the timeout — defined by `concurrent-blocks-and-timers` and `concurrency-completion`.

### Requirement: Select syntax
**Reason**: `select` is removed.
**Migration**: `Concurrent.of(receive_a, receive_b, timer).first()` plus `Channel.try_receive` — defined by `typed-channels` and `concurrency-completion`.

### Requirement: Cancellation shield syntax
**Reason**: `cancellation shield` is removed.
**Migration**: `Concurrent.protect(fn)` — defined by `concurrency-completion`.

## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors, and a distinct removed-construct diagnostic for constructs that were removed from the language.

The words `task`, `await`, `select`, and `scope` / `shield` in a construct position SHALL produce a removed-construct diagnostic that names the replacement (`concurrent { }` / `spawn`, `job.wait()`, `Concurrent.of(...).first()`, `Concurrent.protect`). `scope` and `shield` SHALL otherwise be ordinary identifiers. `parallel` and `thread` remain in the language and are handled by `parallel-cpu-regions` and `concurrency-completion`.

#### Scenario: Removed construct is reported as removed
- **WHEN** `task f()`, `await h`, `select { }`, or `cancellation shield { }` is parsed
- **THEN** the diagnostic states the construct was removed and names its replacement, and is not a bare token error

#### Scenario: Freed word is an identifier
- **WHEN** `mut scope = 1;` or `mut shield = true;` is parsed
- **THEN** `scope` and `shield` are ordinary identifiers

### Requirement: Safety and concurrency grammar

The grammar SHALL parse unsafe function modifiers and blocks and `commit` regions without introducing `async fn`. It SHALL NOT provide `task` blocks, `task scope`, `cancellation shield`, `await` timeouts, or `select` branches; the concurrency surface is `concurrent { }` / `parallel { }` / `spawn`, defined by their own changes.

#### Scenario: Commit region is parsed
- **WHEN** source contains a `commit { }` region inside an unsafe block
- **THEN** the parser produces the commit-region node

#### Scenario: No task or select grammar
- **WHEN** source contains `task { }` or `select { }`
- **THEN** the parser emits the removed-construct diagnostic, not a grammar production

### Requirement: `final` modifier positions

The parser SHALL accept `final` before `class` and in the member-modifier sequence before a method. `final` in any other position (fields, constructors, `interface`, `trait`, `record`, parameters, variables, and any expression position) SHALL produce a targeted diagnostic.

#### Scenario: Final class and method

- **WHEN** `final class A { }` and `class B { final m(): Void { } }` are parsed
- **THEN** the final flags are recorded on the class and the method

#### Scenario: Final field rejected

- **WHEN** a class body contains `final x: Int32;`
- **THEN** a diagnostic is emitted indicating attributes use `inmut`, not `final`
