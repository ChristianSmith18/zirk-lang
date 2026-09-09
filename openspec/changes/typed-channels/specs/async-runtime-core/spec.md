## MODIFIED Requirements

### Requirement: Channel runtime with cooperative suspension
The runtime SHALL provide a channel implementation with a bounded ring buffer or
a growable queue up to a configured limit, a suspended-sender list, and a
suspended-receiver list. A `send` on a full bounded channel SHALL suspend the
sending branch and resume it in first-in-first-out order when capacity frees or
the channel closes. A `receive` on an empty open channel SHALL suspend the
receiving branch. `close` SHALL be idempotent and SHALL wake every suspended
sender and receiver. The executor thread SHALL NOT block on a channel operation;
only the calling branch suspends.

#### Scenario: Sender resumes when capacity frees
- **WHEN** a branch is suspended in `send` on a full channel and another branch receives one value
- **THEN** the suspended sender resumes and enqueues its value

#### Scenario: Close wakes everyone
- **WHEN** a channel with two suspended receivers and one suspended sender is closed
- **THEN** the receivers resume observing drained-closure and the suspended `send` raises

### Requirement: Channel storage is traced by the collector
The collector SHALL trace every value queued in a live channel as a root through
a channel-object trace hook. The channel object SHALL NOT change the shared
object header. The suspended-sender and suspended-receiver lists SHALL NOT be
traced through the channel, because those branches are already roots via the
executor's live-branch enumeration.

#### Scenario: Object reachable only from a queued channel value survives
- **WHEN** the only reference to an object is a value sitting in a channel's queue and a collection runs
- **THEN** the object is intact when the value is later received
