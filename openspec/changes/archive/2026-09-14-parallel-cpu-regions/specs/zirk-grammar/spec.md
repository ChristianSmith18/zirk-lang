## ADDED Requirements

### Requirement: The parallel block grammar

The parser SHALL recognize `parallel` as a keyword that opens a block, optionally
preceded by a `;`-separated option header of `name: expr` pairs between the
keyword and the opening brace (`parallel; cores: 4; chunk: 1000 { ... }`). The
block MAY be used where an expression is expected. Option names (`cores`, `chunk`)
SHALL be contextual and remain usable as ordinary identifiers elsewhere.

#### Scenario: Bare parallel block
- **WHEN** source contains `parallel { for x in xs { work(x); } }`
- **THEN** the parser produces a parallel-block node with an empty option list

#### Scenario: Parallel block with options
- **WHEN** source contains `parallel; cores: -1 { rows.map(f).sum() }`
- **THEN** the parser produces a parallel-block node whose options include `cores` bound to `-1`, usable as an expression

#### Scenario: Option name as an identifier
- **WHEN** `mut cores = 4;` is parsed outside a parallel header
- **THEN** `cores` is an ordinary identifier

## MODIFIED Requirements

### Requirement: Constructs outside the subset

The parser SHALL emit a specific diagnostic for constructs that exist in the language but are not yet implemented, distinguishing them from syntax errors, and a distinct removed-construct diagnostic for constructs that were removed.

`concurrent`, `spawn`, and `parallel` are implemented and parse as real grammar. The removed words `task`, `await`, `select`, `scope`, `shield` keep their removed-construct diagnostic. `thread` remains deferred until its own change.

#### Scenario: Implemented concurrency form parses
- **WHEN** `concurrent { }`, `spawn f()`, or `parallel { }` is parsed
- **THEN** the parser produces the corresponding node and emits no diagnostic

#### Scenario: Removed construct still reported as removed
- **WHEN** `task f()` or `await h` is parsed
- **THEN** the removed-construct diagnostic names the replacement

#### Scenario: Still-deferred construct reported as deferred
- **WHEN** `thread { }` is parsed
- **THEN** the deferred-construct diagnostic names the Phase 5 sub-change that delivers it
