## ADDED Requirements

### Requirement: Output pieces of one line form one logical event

The interpreter SHALL treat consecutive writes to the same standard stream that are not separated by a line break as one logical output line. It SHALL count such a line once against the output-event allowance, and the captured host SHALL deliver the pieces of a line as a single event whose text is their concatenation. Byte accounting, stream separation, and ordering SHALL be unchanged.

#### Scenario: Several writes in one `println`

- **WHEN** an interpreted `println("a", 2)` writes `a`, a space, `2`, and a line break
- **THEN** the host observes one event with text `a 2\n`
- **AND** one output event is counted

#### Scenario: Interleaved streams

- **WHEN** a program writes part of a line to stdout and then to stderr
- **THEN** each stream's text stays in order and the two streams are not merged into one event
