## 1. Spec deltas

- [ ] 1.1 `specs/zirk-structured-concurrency/spec.md` — "Typed channels and closure" for the new surface (this change; drafted)
- [ ] 1.2 `specs/async-runtime-core/spec.md` — "Channel runtime with cooperative suspension" + "Channel storage is traced by the collector" implemented (this change; drafted)
- [ ] 1.3 `specs/zirk-type-system/spec.md`: ADDED — `Channel<T>` known generic; `Channel<T>()` / `(capacity: N)` / `Channel.unbounded<T>(limit: N)` construction; member signatures `send` / `receive: T?` / `try_send: SendOutcome` / `try_receive: ReceiveOutcome<T>` / `close` / `is_closed` / `length` / `capacity` / `drain: List<T>`; `SendOutcome` / `ReceiveOutcome<T>` known enums; `Channel<T>` participates in Share
- [ ] 1.4 `specs/zirk-ir-lowering/spec.md`: ADDED — `ChannelNew`, `ChannelSend`, `ChannelRecv` (safe point), `ChannelTrySend`, `ChannelTryRecv`, `ChannelClose`
- [ ] 1.5 `specs/zirk-native-codegen/spec.md`: ADDED — the channel C-ABI; `ChannelRecv` / backpressured `ChannelSend` emit a suspension point
- [ ] 1.6 `specs/zirk-errors/spec.md`: MODIFIED — closing a channel is never an error; `send` after close raises; `try_*` after close returns the closed outcome
- [ ] 1.7 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 marks `Channel<T>` delivered; `select` stays removed; broadcast/watch/oneshot = follow-up
- [ ] 1.8 `openspec validate typed-channels --strict`

## 2. Runtime (`crates/zirk-runtime/src/channel.rs`)

- [ ] 2.1 New `channel.rs`: `ChannelObject { queue, capacity, limit, closed, senders: Vec<TaskId>, receivers: Vec<TaskId> }`
- [ ] 2.2 `send`: enqueue if room; else register as suspended sender and yield; on resume check cancellation / closed
- [ ] 2.3 `receive`: dequeue if present; else if closed return `None`; else suspend; on resume re-check
- [ ] 2.4 `try_send` / `try_receive`: never suspend; return `SendOutcome` / `ReceiveOutcome`
- [ ] 2.5 `close`: idempotent; wake all suspended senders (their `send` raises) and receivers
- [ ] 2.6 `drain`: `receive` loop until `None`, collect
- [ ] 2.7 Unit tests: rendezvous handoff, bounded backpressure + FIFO resume, unbounded-limit backpressure, close wakes both lists, drain order, drain-before-closure, cancel during suspended send/receive

## 3. Collector

- [ ] 3.1 `crates/zirk-runtime/src/collector.rs`: a channel-object trace hook — mark each queued value; do not walk the waiter lists
- [ ] 3.2 Fixture: the only reference to an object is a queued channel value; a collection runs; the value is intact on receive

## 4. Channel C-ABI + codegen

- [ ] 4.1 `zirk_rt_chan_new(capacity, limit) -> ptr`, `zirk_rt_chan_send`, `zirk_rt_chan_recv`, `zirk_rt_chan_try_send`, `zirk_rt_chan_try_recv`, `zirk_rt_chan_close`, `zirk_rt_chan_len` / `_is_closed` / `_capacity`
- [ ] 4.2 `crates/zirk-codegen-llvm/runtime.rs`: symbols + declarations
- [ ] 4.3 `emit.rs`: `ChannelRecv` and a backpressured `ChannelSend` emit `zirk_rt_suspend` + a resume label; the outcome enums are constructed from the C return
- [ ] 4.4 Integration + golden tests

## 5. Semantic analysis

- [ ] 5.1 `crates/zirk-sema/types.rs`: `Base::Channel(u32)` + table; `SendOutcome` / `ReceiveOutcome<T>` enums
- [ ] 5.2 `checker.rs`: construction forms + member signatures; `receive()` typed `T?`; `Channel<T>` Share (endpoint may be captured by a `spawn` body without consuming)
- [ ] 5.3 Checker tests: construction, member types, `try_*` outcomes, endpoint capture

## 6. IR

- [ ] 6.1 `crates/zirk-ir/ir.rs`: the 6 channel instructions; `verify.rs` (operand types, `ChannelRecv` is a safe point)
- [ ] 6.2 `lower.rs`: channel construction + method calls -> instructions
- [ ] 6.3 IR golden tests

## 7. Fixtures + example

- [ ] 7.1 `valid/channel_rendezvous.zrk`, `valid/channel_bounded_backpressure.zrk`, `valid/channel_worker_pool.zrk`, `valid/channel_drain.zrk`, `valid/channel_close_wakes.zrk`
- [ ] 7.2 `invalid/channel_send_after_close.zrk`
- [ ] 7.3 `examples/channel_examples.zrk`: producer/consumer, worker pool draining a queue, streaming fan-out results; compile-and-run CLI test

## 8. Documentation

- [ ] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: rewrite the channel section for the new surface; note `select` is removed and multi-source waiting is `Concurrent.of(...).first()`
- [ ] 8.2 Handbook: channel chapter + `Channel<T>` reference page
- [ ] 8.3 `docs/init/ZIRK_ROADMAP.md` + `ZIRK_FEATURE_STATUS.md`: `Channel<T>` delivered; broadcast/watch/oneshot = follow-up
- [ ] 8.4 `README.md` concurrency line

## 9. Website + closeout

- [ ] 9.1 `cargo test --workspace` green; fmt; clippy
- [ ] 9.2 Commit; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review status catalog; commit `../zirk-lang-site` separately; record both revisions
- [ ] 9.3 `openspec validate typed-channels --strict`
