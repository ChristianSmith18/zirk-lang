## MODIFIED Requirements

### Requirement: Typed channels and closure
`Channel<T>` SHALL support explicit bounded construction, zero-capacity
rendezvous, a defensively limited growable construction, suspendible
`send` / `receive` that are cancellation safe points, non-blocking
`try_send` / `try_receive` with typed outcomes, idempotent `close`, sender
backpressure on a full bounded channel, and an unambiguous distinction between a
value, closure, failure, and temporary absence. Queued values SHALL drain before
closure is observed. `channel.receive()` SHALL yield `Some(v)` while values
remain and `None` once a closed channel is drained. `channel.drain()` SHALL
receive until the channel is closed and drained and return the values in order.
Standard broadcast, latest-value watch, and one-shot channel families remain
specified and are delivered by a follow-up change.

#### Scenario: Bounded channel is full
- **WHEN** a sender uses suspendible `send` on a full bounded channel
- **THEN** the sender suspends without blocking the executor thread until capacity or closure is observed

#### Scenario: Growable channel reaches its defense limit
- **WHEN** a growable channel reaches its configured limit
- **THEN** sending applies backpressure or returns the documented typed failure rather than allocating without bound

#### Scenario: Closed channel drains before reporting closure
- **WHEN** a channel with three queued values is closed and a branch calls `receive` four times
- **THEN** the first three calls return the queued values in order and the fourth returns `None`

#### Scenario: try operations distinguish empty from closed
- **WHEN** `try_receive` is called on an open empty channel and again after it is closed and drained
- **THEN** the first returns `Empty` and the second returns `Closed`

#### Scenario: Cancellation interrupts a suspended receive
- **WHEN** a branch is cancelled while suspended in `channel.receive()`
- **THEN** `receive` raises `CancelledError` at that point
