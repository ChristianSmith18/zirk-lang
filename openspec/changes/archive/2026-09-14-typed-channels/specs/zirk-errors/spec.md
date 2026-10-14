## ADDED Requirements

### Requirement: Channel closure is not an error

`channel.close()` SHALL never raise. A `send` on a closed channel SHALL raise a
compiler-known channel-closed failure; a `try_send` on a closed channel SHALL
return `SendOutcome.Closed`; a `receive` on a drained closed channel SHALL return
`None`; a `try_receive` on a drained closed channel SHALL return
`ReceiveOutcome.Closed`. A branch suspended in `send` or `receive` when the
channel closes SHALL resume observing closure, and a branch cancelled while
suspended in a channel operation SHALL raise `CancelledError`.

#### Scenario: send after close raises
- **WHEN** `ch.send(v)` is called after `ch.close()`
- **THEN** a channel-closed failure is raised

#### Scenario: try_send after close returns Closed
- **WHEN** `ch.try_send(v)` is called after `ch.close()`
- **THEN** it returns `SendOutcome.Closed` and does not raise

#### Scenario: suspended receiver observes closure
- **WHEN** a branch is suspended in `ch.receive()` and another branch closes `ch`
- **THEN** the suspended branch resumes and `receive` returns the remaining queued values then `None`
