## Why

`concurrent-blocks-and-timers` gives branches structured scopes but no way to
talk to each other mid-flight. Producer/consumer pipelines, worker pools draining
a queue, and streaming results out of a fan-out all need a typed conduit.

`Channel<T>` is already specified in `zirk-structured-concurrency` ("Typed
channels and closure") and `async-runtime-core` ("Channel runtime with
cooperative suspension", "Channel storage is traced by the collector"), but those
requirements were written for the `task` / `select` model. This change implements
`Channel<T>` on the new surface: no `select` (multi-source waiting is
`Concurrent.of(...).first()` over receive thunks, defined in
`concurrency-completion`), cooperative suspension on the single-threaded executor,
and collector tracing of queued values.

Change #4 of five. Depends on #1 and #2.

## What Changes

- **`Channel<T>` construction**: `Channel<T>()` unbounded (with a mandatory
  internal defense limit), `Channel<T>(capacity: N)` bounded, `Channel<T>(capacity:
  0)` rendezvous, `Channel.unbounded<T>(limit: N)` an explicitly bounded growable
  channel.
- **Blocking API**: `channel.send(v)` and `channel.receive() -> T?` suspend the
  calling branch cooperatively and are cancellation safe points. A full bounded
  channel applies backpressure to senders; `receive` on an empty open channel
  suspends; `receive` on a drained closed channel returns `None`.
- **Non-blocking API**: `channel.try_send(v) -> SendOutcome`, `channel.try_receive()
  -> ReceiveOutcome` — never suspend; typed outcomes distinguish success,
  temporary full/empty, and closed.
- **Closure**: `channel.close()` is idempotent, wakes suspended operations,
  and lets queued values drain before `receive` reports closure. `channel.is_closed`,
  `channel.length`, `channel.capacity`.
- **`channel.drain() -> List<T>`**: a convenience terminal — receive until closed
  and drained, collect in order. (New; not in the old spec.)
- **Collector tracing**: queued channel values are roots; the channel runtime
  object gets a custom trace hook (an ordinary collector extension, no header
  change).
- **Channel families** (`broadcast`, `watch`, `oneshot`) are specified but
  **deferred** to a follow-up; this change delivers the core `Channel<T>`.

## Capabilities

### New Capabilities

_None._ Implements requirements already in `zirk-structured-concurrency` and
`async-runtime-core`, restated for the new surface.

### Modified Capabilities

- `zirk-structured-concurrency`: "Typed channels and closure" — drop the
  `select` / broadcast / watch / oneshot obligations from *this* change's scope
  (kept in the spec, marked follow-up); keep bounded / rendezvous / growable /
  suspendible / try / idempotent-close / backpressure / drain-before-closure.
- `zirk-type-system`: `Channel<T>` is a known one-parameter generic; the
  `send` / `receive` / `try_*` / `close` / `drain` member signatures;
  `SendOutcome` / `ReceiveOutcome` known enums; `Channel<T>` participates in
  Transfer/Share (a channel endpoint may cross a branch boundary).
- `zirk-ir-lowering`: `ChannelNew`, `ChannelSend`, `ChannelRecv` (a safe point),
  `ChannelTrySend`, `ChannelTryRecv`, `ChannelClose` instructions.
- `zirk-native-codegen`: the channel C-ABI (`zirk_rt_chan_new` / `send` / `recv`
  / `try_send` / `try_recv` / `close`); `ChannelRecv` emits a suspension point.
- `async-runtime-core`: "Channel runtime with cooperative suspension" and
  "Channel storage is traced by the collector" — implement, drop `select`
  integration.
- `zirk-feature-phasing`: Phase 5 marks `Channel<T>` delivered; `select` stays
  removed; broadcast/watch/oneshot listed as follow-up.
- `zirk-errors`: closing a channel with suspended senders/receivers is not an
  error; a `try_*` on a closed channel returns the closed outcome, never throws.

## Impact

- **Code**: `crates/zirk-sema` (`Base::Channel`, member signatures, outcome
  enums, Transfer/Share for endpoints), `crates/zirk-ir` (5 instructions),
  `crates/zirk-codegen-llvm` (channel C-ABI, `ChannelRecv` suspension),
  `crates/zirk-runtime` (`channel.rs` — bounded/unbounded queue, waiter lists,
  backpressure, idempotent close, drain-before-closure; `collector.rs` — custom
  trace hook for the channel object). Fixtures + unit tests.
- **Runtime**: `channel.rs` is new; `collector.rs` gains one trace hook.
- **Normative docs**: `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md` channel section
  (rewrite for the new surface, note `select` is gone), handbook channel
  chapter + reference page, `examples/channel_examples.zrk`.
- **Companion `../zirk-lang-site`**: channel chapter, examples, Phase 5 status.
  Sync with a reviewed `--audit-date`.
