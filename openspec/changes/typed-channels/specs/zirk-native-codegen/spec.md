## ADDED Requirements

### Requirement: Codegen for channel operations

Native code generation SHALL emit calls to the channel C-ABI: `zirk_rt_chan_new`,
`zirk_rt_chan_send`, `zirk_rt_chan_recv`, `zirk_rt_chan_try_send`,
`zirk_rt_chan_try_recv`, `zirk_rt_chan_close`, and the accessors. A `ChannelRecv`
and a backpressured `ChannelSend` SHALL emit a runtime suspend call plus a resume
label. The `SendOutcome` / `ReceiveOutcome` enum values SHALL be constructed from
the C return code.

#### Scenario: receive emits a suspension point
- **WHEN** codegen processes a `ChannelRecv`
- **THEN** the emitted code calls `zirk_rt_chan_recv` through the suspend path and defines a resume label

#### Scenario: try_receive is a plain call
- **WHEN** codegen processes a `ChannelTryRecv`
- **THEN** the emitted code calls `zirk_rt_chan_try_recv` and builds a `ReceiveOutcome` from the result with no resume label
