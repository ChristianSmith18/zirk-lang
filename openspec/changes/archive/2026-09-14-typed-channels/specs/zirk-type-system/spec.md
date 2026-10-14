## ADDED Requirements

### Requirement: Channel type and members

`Channel<T>` SHALL be a known one-parameter generic type. `Channel<T>()` SHALL
construct an unbounded channel with an internal defense limit; `Channel<T>(capacity:
N)` a bounded channel; `Channel<T>(capacity: 0)` a rendezvous channel;
`Channel.unbounded<T>(limit: N)` an explicitly bounded growable channel. Member
typing: `send(v: T): Void`, `receive(): T?`, `try_send(v: T): SendOutcome`,
`try_receive(): ReceiveOutcome<T>`, `close(): Void`, `is_closed: Boolean`,
`length: Int`, `capacity: Int`, `drain(): List<T>`. `SendOutcome` (`Sent` /
`Full` / `Closed`) and `ReceiveOutcome<T>` (`Received(T)` / `Empty` / `Closed`)
SHALL be known enums. A `Channel<T>` value SHALL participate in `Share`: it MAY be
held and used concurrently by more than one branch and MAY be captured by a
`spawn` body without being consumed.

#### Scenario: channel construction and receive type
- **WHEN** `inmut ch = Channel<Int32>(capacity: 8);` is checked
- **THEN** `ch` has type `Channel<Int32>` and `ch.receive()` has type `Int32?`

#### Scenario: try_receive distinguishes empty from closed
- **WHEN** `ch.try_receive()` is checked
- **THEN** its type is `ReceiveOutcome<Int32>` with variants `Received`, `Empty`, `Closed`

#### Scenario: endpoint captured by a branch is not consumed
- **WHEN** a `spawn` body captures `ch` and the enclosing scope also uses `ch`
- **THEN** the check succeeds because `Channel<T>` is `Share`
