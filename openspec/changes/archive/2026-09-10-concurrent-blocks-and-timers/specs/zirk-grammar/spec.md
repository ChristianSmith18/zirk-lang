## ADDED Requirements

### Requirement: The concurrent block

The parser SHALL recognize `concurrent { ... }` as a statement. Inside it, `inmut`
and `mut` declarations are branch bindings, plain statements are the block-body
branch, and `spawn` expressions are dynamic branches. The parser SHALL expose the
block's top-level binding list so the checker can build the dependency DAG.

#### Scenario: Concurrent block with named branches
- **WHEN** source contains `concurrent { inmut a = f(); inmut b = g(); }`
- **THEN** the parser produces a concurrent-block node with `a` and `b` as branch bindings

#### Scenario: Concurrent block with a spawn loop
- **WHEN** a `concurrent` block body contains `for x in xs { spawn h(x); }`
- **THEN** the parser records the `spawn` calls as dynamic branches of the block

### Requirement: The spawn expression

The parser SHALL recognize `spawn <call>` and `spawn { block }` as a prefix
expression, bindable (`inmut h = spawn f()`) or used as a statement (`spawn f()`).

#### Scenario: Spawn with a handle
- **WHEN** source contains `inmut h = spawn compute();`
- **THEN** the parser produces a binding whose initializer is a spawn node over `compute()`

## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors, and a distinct removed-construct diagnostic for constructs that were removed.

`concurrent` and `spawn` are implemented and parse as real grammar. The removed words `task`, `await`, `select`, `scope`, `shield` keep their removed-construct diagnostic. `parallel`, `thread` remain deferred until their own change.

#### Scenario: Implemented concurrency form parses
- **WHEN** `concurrent { }` or `spawn f()` is parsed
- **THEN** the parser produces the corresponding node and emits no diagnostic

#### Scenario: Removed construct still reported as removed
- **WHEN** `task f()` or `await h` is parsed
- **THEN** the removed-construct diagnostic names the replacement
