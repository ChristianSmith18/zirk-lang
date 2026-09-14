## ADDED Requirements

### Requirement: Codegen for channel operations

Native code generation SHALL emit calls to the channel C-ABI: `zirk_rt_chan_new`,
`zirk_rt_chan_send`, `zirk_rt_chan_recv`, `zirk_rt_chan_try_send`,
`zirk_rt_chan_try_recv`, `zirk_rt_chan_close`, and the accessors. In the
stackful executor model, `ChannelRecv` and a backpressured `ChannelSend` SHALL
use the blocking runtime calls as cooperative suspension points; the runtime
parks and later resumes the current branch, so no separate `zirk_rt_suspend`
symbol is required. The `SendOutcome` / `ReceiveOutcome` enum values SHALL be
constructed from the C return code.

Construction SHALL provide the element byte size, alignment, and GC-reference
offset metadata. Send and receive SHALL carry source or destination element
slots indirectly, preserving the native representation of scalar, reference,
and aggregate `T` values without boxing.

#### Scenario: receive emits a suspension point
- **WHEN** codegen processes a `ChannelRecv`
- **THEN** the emitted code calls `zirk_rt_chan_recv` through the stackful
  cooperative path and the generated function is unwind-capable

#### Scenario: try_receive is a plain call
- **WHEN** codegen processes a `ChannelTryRecv`
- **THEN** the emitted code calls `zirk_rt_chan_try_recv` and builds a `ReceiveOutcome` from the result with no resume label
