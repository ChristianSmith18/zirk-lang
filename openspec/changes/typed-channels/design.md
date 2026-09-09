## Context

`Channel<T>` was specified for the old model with `select` as its multiplexer.
The new model has no `select`; multi-source waiting is
`Concurrent.of(receive_thunk_a, receive_thunk_b, timer_thunk).first()`, defined
in `concurrency-completion`. This change implements the core channel on the
single-threaded cooperative executor.

## Goals / Non-Goals

**Goals**: `Channel<T>` construction (unbounded / bounded / rendezvous /
growable-with-limit), blocking `send` / `receive`, non-blocking `try_send` /
`try_receive` with typed outcomes, idempotent `close` with drain-before-closure,
`drain()`, collector tracing of queued values, Transfer/Share for endpoints.

**Non-Goals**: `select` (removed), `broadcast` / `watch` / `oneshot` families
(specified, deferred), multi-threaded channel access (the executor is one
thread; `parallel` regions do not touch channels — #3 rejects that).

## Decisions

### D1: `receive` returns `T?`, not a dedicated enum

`channel.receive()` yields `Some(v)` while values remain (even after close) and
`None` once a closed channel is drained. This reuses the nullable machinery and
reads naturally in a `loop { match ch.receive() { Some(v) => ..., None => break } }`.

**Alternative**: a `RecvResult` enum with `Value` / `Closed`. Rejected — `T?` is
already in the language and the drained-closed case maps cleanly to `None`.

### D2: `try_*` return typed outcome enums

`try_send(v) -> SendOutcome { Sent, Full, Closed }`, `try_receive() ->
ReceiveOutcome<T> { Received(T), Empty, Closed }`. Distinct from `receive`'s `T?`
because `try_receive` must distinguish "empty right now" from "closed".

### D3: Blocking ops are cancellation safe points

`send` (when backpressured) and `receive` (when suspended) check the branch's
cancellation flag on resume and raise `CancelledError`. This is the same
safe-point mechanism `Timer.sleep` uses (#2).

### D4: `close` semantics

Idempotent. On close: wake every suspended sender with `CannotSend` /
`CancelledError` semantics (a suspended `send` on a now-closed channel raises),
wake every suspended receiver. Queued values stay receivable; after the queue
drains, `receive` returns `None` and `try_receive` returns `Closed`. `send` after
close raises; `try_send` after close returns `Closed`.

### D5: Collector trace hook

The channel runtime object holds a ring buffer / linked queue of `T` values plus
two waiter lists of `TaskId`. The collector's mark phase, on reaching a channel
object, traces every queued value as a root and does not trace the waiter lists
(those `TaskId`s are already roots via the executor's live-branch iteration).
One `register_trace_hook`-style extension in `collector.rs`; no header change
(`ADR-012`).

### D6: Endpoints and Transfer/Share

A `Channel<T>` value is a shared handle; both a sender and a receiver branch hold
it. It participates in Share (multiple branches may hold and use it
concurrently). Passing a `Channel<T>` into a `spawn` body is allowed and does not
consume it.

## Risks / Trade-offs

- **Unbounded channel memory** → the mandatory internal defense limit
  (`Channel.unbounded(limit:)` makes it explicit; the bare `Channel<T>()` uses a
  documented default limit) turns unbounded growth into backpressure or a typed
  failure.
- **`drain()` on a never-closed channel hangs** → document it as "receive until
  closed"; it is the caller's job to ensure a producer closes.
- **Cancellation during `send` backpressure** → the value is not enqueued; the
  `send` raises; document that a cancelled `send` did not deliver.
- **A `parallel` region touching a channel** → statically rejected by #3
  (`PARALLEL_REGION_IO`); a channel op is a safe point and pool threads have
  none.

## Migration Plan

1. Spec deltas; `openspec validate`.
2. `channel.rs`: queue + waiter lists + backpressure + idempotent close +
   drain-before-closure; unit tests (rendezvous, bounded, unbounded-limit,
   close wakes, drain order, cancel during send/receive).
3. `collector.rs`: the channel trace hook; a fixture holding the only reference
   to an object inside a queued channel value across a collection.
4. Channel C-ABI + codegen (`ChannelRecv` = suspension point).
5. Sema: `Base::Channel`, member signatures, outcome enums, Share for endpoints.
6. IR: the 5 instructions; lowering; `verify.rs`.
7. Fixtures + `examples/channel_examples.zrk`.
8. Docs: semantics channel section, handbook chapter + reference, roadmap,
   feature status.
9. `cargo test` + fmt + clippy; commit; website sync with reviewed date.

## Open Questions

- Bare `Channel<T>()` default defense limit value — proposed: a large documented
  constant (e.g. 2^16 queued), overridable via `Channel.unbounded(limit:)`.
- `drain()` — keep as a convenience, or force `loop { receive() }`? Proposed:
  keep; it is the single most common channel consumer shape.
- Should `Channel<T>` be must-use (a channel created and never closed leaks a
  producer)? Proposed: not must-use in this change; revisit if leaks show up.
