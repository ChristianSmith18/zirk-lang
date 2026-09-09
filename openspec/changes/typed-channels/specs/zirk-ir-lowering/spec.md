## ADDED Requirements

### Requirement: Lowering of channel operations

Channel construction SHALL lower to `ChannelNew` carrying capacity and limit.
`send` SHALL lower to `ChannelSend`, a suspension point when the channel is a full
bounded channel. `receive` SHALL lower to `ChannelRecv`, a suspension point when
the channel is empty and open. `try_send` / `try_receive` SHALL lower to
`ChannelTrySend` / `ChannelTryRecv`, never suspension points. `close` SHALL lower
to `ChannelClose`. Every channel suspension point SHALL include a cancellation
check that raises `CancelledError` when cancellation is pending.

#### Scenario: receive lowers to a suspension point
- **WHEN** `ch.receive()` is lowered
- **THEN** the IR contains a `ChannelRecv` with a resume label and a cancellation check

#### Scenario: try_receive does not suspend
- **WHEN** `ch.try_receive()` is lowered
- **THEN** the IR contains a `ChannelTryRecv` with no resume label
