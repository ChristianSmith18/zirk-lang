# zirk-standard-streams Specification

## Purpose

Defines the complete portable public contract for standard input, standard
output, and standard error as the `std.io` stream modules.

## Requirements

#

#

#

#

#

### Requirement: std.io publishes complete standard stream modules

`std.io` SHALL publish `stdin`, `stdout`, and `stderr` as ordinary Zirk
modules. Importing a stream module SHALL open its shared members when their
names are unambiguous, and explicit stream calls SHALL use dot access.

#### Scenario: Open standard output
- **WHEN** source imports `stdout` from `std.io`
- **THEN** both `println("ready")` and `stdout.println("ready")` are valid

#### Scenario: Import a stream member directly
- **WHEN** source imports `println` from `std.io::stdout`
- **THEN** `println("ready")` is valid and `stdout` is not introduced

### Requirement: stdout and stderr provide deterministic text output

`stdout` and `stderr` SHALL provide `print(...args: T[]): Void`,
`println(...args: T[]): Void`, and `flush(): Result<Void, IoError>`. Print
operations SHALL render values through `to_string()`, join arguments with one
space, and preserve call order. `println` SHALL append exactly one platform
line ending; `print()` SHALL emit no bytes and `println()` only that ending.

#### Scenario: Render multiple values
- **WHEN** `stdout.println("count", 2)` executes
- **THEN** standard output receives `count 2` followed by one line ending

#### Scenario: Keep standard error separate
- **WHEN** `stderr.print("warning")` executes
- **THEN** the text is delivered to standard error without a line ending

### Requirement: stdin distinguishes value, end, and failure

`stdin` SHALL provide `read_line(): String?`, `read_line_result(): ReadResult<String>`,
`read_char(): ReadResult<Char>`, `read_bytes(max: UInt64): ReadResult<Array<Byte>>`,
and `read_exact(count: UInt64): Result<Array<Byte>, IoError>`. A line read
SHALL remove its terminating line ending. An empty line SHALL be `Value("")`;
EOF SHALL be `End`; host and decoding failures SHALL be `Error(IoError)`.

#### Scenario: Empty line is not EOF
- **WHEN** stdin supplies an empty terminated line
- **THEN** `read_line_result()` returns `ReadResult.Value("")`

#### Scenario: EOF through the convenience API
- **WHEN** stdin is at EOF before a line begins
- **THEN** `read_line()` returns `null`

### Requirement: standard stream input remains bounded and Unicode-safe

Text input SHALL decode strict UTF-8. `read_bytes(max)` SHALL reject a request
above the configured stream bound with `ReadResult.Error(IoError.Limit)`, and
`read_exact(count)` SHALL return `IoError.EndOfStream` when fewer bytes remain.

#### Scenario: Invalid UTF-8 input
- **WHEN** a text read encounters invalid UTF-8
- **THEN** it returns `ReadResult.Error(IoError.Decoding)` without replacing bytes

#### Scenario: Exact read reaches EOF early
- **WHEN** `read_exact(4)` has only two bytes available
- **THEN** it returns `Error(IoError.EndOfStream)`

### Requirement: browser execution exposes output but not ambient input

Browser-compatible execution SHALL capture stdout and stderr as ordered,
bounded output events. It SHALL reject reachable stdin read operations with a
capability diagnostic before execution and SHALL not request browser input.

#### Scenario: Browser output
- **WHEN** browser-compatible source calls `stdout.println("ready")`
- **THEN** its structured result contains the output event in call order

#### Scenario: Browser input request
- **WHEN** browser-compatible source reaches `stdin.read_line()`
- **THEN** execution stops with an unavailable-input capability result
