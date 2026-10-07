## ADDED Requirements

### Requirement: Execution trace is an opt-in, bounded, read-only observation channel

When tracing is enabled, the interpreter SHALL emit an ordered trace of object allocation, field write, method call, method return, and throw events, each stamped with a sequence number that also orders output events. Allocation events SHALL carry the class name and a per-run object id assigned in allocation order. Field-write events SHALL carry the object id, field name, declared type, and a rendered value bounded in length. The trace SHALL be bounded by configured event and byte limits, SHALL NOT be readable by the interpreted program, and SHALL NOT change the program's output, completion, failure category, or instruction-limit behavior.

#### Scenario: Object lifecycle is traced
- **WHEN** a traced program constructs an object and assigns one of its fields
- **THEN** the trace contains an allocation event for that object followed by a field-write event naming the field
- **AND** both carry increasing sequence numbers

#### Scenario: Trace interleaves with output
- **WHEN** a traced program prints between two field writes
- **THEN** the output event's sequence number falls between the two field-write sequence numbers

#### Scenario: Object-valued field renders as a reference
- **WHEN** a traced program assigns an object to a field of another object
- **THEN** the field-write event renders the value as a reference to the target's object id and does not render the target's contents

#### Scenario: Tracing does not change behavior
- **WHEN** the same program runs with tracing enabled and disabled
- **THEN** output, completion, failure category, and the instruction count at which a limit trips are identical

#### Scenario: Trace budget is exhausted
- **WHEN** the number of events or bytes would exceed the configured trace limit
- **THEN** recording stops and the trace is marked truncated
- **AND** the program continues to run normally

#### Scenario: Object ids are deterministic
- **WHEN** a program runs twice with identical input and limits
- **THEN** both traces contain identical object ids and sequence numbers
