## MODIFIED Requirements

### Requirement: Standard output

Native runtime execution SHALL expose stable `extern "C"` functions that write a `String` to standard output with the requested newline behavior. Interpreted execution SHALL emit the same logical UTF-8 text and newline sequence through an injected bounded host output sink without requiring a native process stream.

#### Scenario: Print through the native runtime
- **WHEN** a native program invokes the print function with a `String`
- **THEN** the content appears on native standard output with the requested newline behavior

#### Scenario: Print through the interpreter
- **WHEN** an interpreted program invokes `stdout.print` or `stdout.println`
- **THEN** the content is emitted to the host output sink in call order
- **AND** `println` appends exactly one newline

#### Scenario: Non-ASCII content
- **WHEN** either execution engine prints Unicode characters outside ASCII
- **THEN** the host receives them correctly encoded in UTF-8

#### Scenario: Flush native output before terminating
- **WHEN** a native program terminates
- **THEN** standard output is flushed before the process ends

#### Scenario: Complete interpreted output before returning
- **WHEN** an interpreted program terminates
- **THEN** all accepted output events are present in the structured execution result before it is returned

#### Scenario: Bound interpreted output
- **WHEN** an interpreted output event would exceed the configured host sink limit
- **THEN** the interpreter reports output truncation and a controlled limit failure
- **AND** it does not allocate or emit bytes beyond that limit
