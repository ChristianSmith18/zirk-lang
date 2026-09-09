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
