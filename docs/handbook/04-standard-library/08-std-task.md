# `std.task`

`std.task` supports the language's structured `task`, `await` and `select`; it
does not introduce `async fn` or a public event loop. Every child belongs to a
scope, and scope exit completes or cancels and cleans children before returning.

> **Implementation status:** accepted Zirk 1.x concurrency contract.

## Handles and consumption

```zirk
inmut operation: Task<User> = task(name: 'load-user') load_user(id);
inmut user = await operation;
```

Creation starts the child immediately. `Task<T>` is consumed by its single
`await`; awaiting it again is a compile-time use-after-consume error. Multiple
consumers use `Watch`, `Broadcast`, channels or an explicitly shared deeply
immutable result.

Task names are diagnostic metadata. Scheduling priority is automatic and not a
user-settable API. Ignoring a task is diagnosed. Detachment is forbidden;
long-lived services transfer ownership to an explicit application/service scope.

## Cancellation and aggregation

```zirk
operation.cancel();
operation.cancel(reason: CancellationReason.UserRequest);
```

The default reason is `Cancelled`; cancellation is idempotent and cooperative.
`Task.all` preserves input order and cancels siblings on the first unhandled
throwable. `Task.first` returns the first completion and cleans the rest.
`Task.settled` lets every child finish and returns ordered `Fulfilled`,
`Rejected` or `Cancelled` outcomes. A `Result.Error` remains a fulfilled task
value rather than an unhandled throwable.

`task.blocking` runs unavoidable blocking code on a runtime-managed bounded
pool. Saturation waits cancelably instead of creating unlimited threads; pool
configuration is an advanced `.zkinit` setting.

## Channels

Bounded capacity is explicit and creates backpressure:

```zirk
inmut messages = Channel<Message>(capacity: 100);
inmut rendezvous = Channel<Message>.rendezvous();
```

`Channel.unbounded(limit:)` grows dynamically only up to its mandatory defense
limit. `send` after close returns `Result.Error(ChannelClosed)`. Close is
idempotent; receivers drain queued values before observing:

```zirk
enum ReceiveResult<T> {
    Value(T);
    Closed;
    Error(ChannelError);
}
```

`Broadcast<T>` sends to each subscriber, `Watch<T>` retains the latest value,
and `OneShot<T>` permits one send/receive. Transfer/share checks apply at every
boundary. `select` uses fair rotating selection among simultaneously ready
branches, supports `after` timeouts and never starves a permanently ready peer.

Task-local application state is not a general global facility; arguments,
captures and typed task context keep dependencies visible. Runtime tracing may
use internal context without exposing an ambient event loop.

---

**Previous:** [← std.time](07-std-time.md) · **Next:** [ std.thread](09-std-thread.md)
