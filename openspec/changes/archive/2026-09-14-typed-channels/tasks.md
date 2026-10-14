## 1. Spec deltas

- [x] 1.1 `specs/zirk-structured-concurrency/spec.md` — "Typed channels and closure" for the new surface (this change; drafted)
- [x] 1.2 `specs/async-runtime-core/spec.md` — "Channel runtime with cooperative suspension" + "Channel storage is traced by the collector" implemented (this change; drafted)
- [x] 1.3 `specs/zirk-type-system/spec.md`: ADDED — `Channel<T>` known generic; `Channel<T>()` / `(capacity: N)` / `Channel.unbounded<T>(limit: N)` construction; member signatures `send` / `receive: T?` / `try_send: SendOutcome` / `try_receive: ReceiveOutcome<T>` / `close` / `is_closed` / `length` / `capacity` / `drain: List<T>`; `SendOutcome` / `ReceiveOutcome<T>` known enums; `Channel<T>` participates in Share
- [x] 1.4 `specs/zirk-ir-lowering/spec.md`: ADDED — `ChannelNew`, `ChannelSend`, `ChannelRecv` (safe point), `ChannelTrySend`, `ChannelTryRecv`, `ChannelClose`
- [x] 1.5 `specs/zirk-native-codegen/spec.md`: ADDED — the channel C-ABI; `ChannelRecv` / backpressured `ChannelSend` emit a suspension point
- [x] 1.6 `specs/zirk-errors/spec.md`: MODIFIED — closing a channel is never an error; `send` after close raises; `try_*` after close returns the closed outcome
- [x] 1.7 `specs/zirk-feature-phasing/spec.md`: MODIFIED — Phase 5 marks `Channel<T>` delivered; `select` stays removed; broadcast/watch/oneshot = follow-up
- [x] 1.8 `openspec validate typed-channels --strict`

## 2. Runtime (`crates/zirk-runtime/src/channel.rs`)

- [x] 2.1 New `channel.rs`: `ChannelObject { queue, capacity, limit, closed, senders: Vec<TaskId>, receivers: Vec<TaskId> }`
- [x] 2.2 `send`: enqueue if room; else register as suspended sender and yield; on resume check cancellation / closed
- [x] 2.3 `receive`: dequeue if present; else if closed return `None`; else suspend; on resume re-check
- [x] 2.4 `try_send` / `try_receive`: never suspend; return `SendOutcome` / `ReceiveOutcome`
- [x] 2.5 `close`: idempotent; wake all suspended senders (their `send` raises) and receivers
- [x] 2.6 `drain`: `receive` loop until `None`, collect
- [x] 2.7 Unit tests: rendezvous handoff, bounded backpressure + FIFO resume, unbounded-limit backpressure, close wakes both lists, drain order, drain-before-closure, cancel during suspended send/receive

## 3. Collector

- [x] 3.1 `crates/zirk-runtime/src/collector.rs`: a channel-object trace hook — mark each queued value; do not walk the waiter lists
- [x] 3.2 Fixture: the only reference to an object is a queued channel value; a collection runs; the value is intact on receive

## 4. Channel C-ABI + codegen

- [x] 4.1 `zirk_rt_chan_new(capacity, limit) -> ptr`, `zirk_rt_chan_send`, `zirk_rt_chan_recv`, `zirk_rt_chan_try_send`, `zirk_rt_chan_try_recv`, `zirk_rt_chan_close`, `zirk_rt_chan_len` / `_is_closed` / `_capacity`
- [x] 4.2 `crates/zirk-codegen-llvm/runtime.rs`: symbols + declarations
- [x] 4.3 `emit.rs`: `ChannelRecv` and a backpressured `ChannelSend` use the runtime's stackful cooperative suspension path; `ChannelSend` checks `CHAN_CLOSED` and routes it through the compiler-known failure machinery; typed outcomes are constructed from the C return
- [x] 4.4 Integration + golden tests

## 5. Semantic analysis

- [x] 5.1 `crates/zirk-sema/types.rs`: `Base::Channel(u32)` + table; `SendOutcome` / `ReceiveOutcome<T>` enums
- [x] 5.2 `checker.rs`: construction forms + member signatures; `receive()` typed `T?`; `Channel<T>` Share (endpoint may be captured by a `spawn` body without consuming)
- [x] 5.3 Checker tests: construction, member types, `try_*` outcomes, endpoint capture

## 6. IR

- [x] 6.1 `crates/zirk-ir/ir.rs`: the 6 channel instructions; `verify.rs` (operand types, `ChannelRecv` is a safe point)
- [x] 6.2 `lower.rs`: channel construction + method calls -> instructions
- [x] 6.3 IR golden tests

## 7. Fixtures + example

- [x] 7.1 `valid/typed_channel_basic.zrk` (construction, try/send/receive, close smoke fixture)
- [x] 7.2 `invalid/channel_send_after_close.zrk`
- [x] 7.3 `examples/channel_examples.zrk`: producer/consumer, worker pool draining a queue, streaming fan-out results; compile-and-run CLI test

## 8. Documentation

- [x] 8.1 `docs/STRUCTURED_CONCURRENCY_SEMANTICS.md`: rewrite the channel section for the new surface; note `select` is removed and multi-source waiting is `Concurrent.of(...).first()`
- [x] 8.2 Handbook: channel chapter + `Channel<T>` reference page
- [x] 8.3 `docs/init/ZIRK_ROADMAP.md` + `ZIRK_FEATURE_STATUS.md`: `Channel<T>` delivered; broadcast/watch/oneshot = follow-up
- [x] 8.4 `README.md` concurrency line

## 9. Website + closeout

- [x] 9.1 `cargo test --workspace` green; fmt; clippy
- [x] 9.2 Commit; `./scripts/sync-website-content.sh --audit-date YYYY-MM-DD`; review status catalog; commit `../zirk-lang-site` separately; record both revisions (`zirk-lang`: `27342fa`, `zirk-lang-site`: `3d82c99`)
- [x] 9.3 `openspec validate typed-channels --strict`

## 10. Mitigations

- [x] 10.1 Preserve nested GC references through `Channel<T>.drain()` by carrying complete reference offsets into list backing-buffer descriptors.
- [x] 10.2 Release dynamic array/list descriptors and list-owned layout metadata during collector sweep.
- [x] 10.3 Avoid copying a send value before a full bounded channel has accepted it; retain one copy only for a suspended sender.
- [x] 10.4 Propagate blocking-send closure as a compiler-known failure while preserving non-throwing `try_send` closure status.
- [x] 10.5 Validate stackful channel suspension, cancellation, closure wake-up, FIFO order, and generated ABI paths.
- [x] 10.6 Run the full workspace/GC stress matrix and record any residual performance or leak evidence.
