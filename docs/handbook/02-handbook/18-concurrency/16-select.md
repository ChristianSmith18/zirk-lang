# `select`

`select` waits for the first ready task, channel operation, timer, or
cancellation signal and executes exactly one branch:

```zirk
select {
    message = await messages.receive() => process(message),
    result = await operation => finish(result),
    after 5s => report_timeout(),
    cancelled => cleanup(),
}
```

Without `default`, selection suspends without blocking an OS thread. With
`default`, it returns immediately when no guarded operation is ready. Multiple
ready branches are chosen fairly rather than permanently by source order.

Losing operations remain alive; `select` does not cancel them automatically. A
branch can cancel work explicitly. Channel closure counts as a ready typed
outcome, and selected failures propagate normally unless handled in the branch.

---

**Previous:** [← Task Aggregation](15-task-aggregation.md) · **Next:** [ Transfer, Sharing, and Captures](17-transfer-sharing-and-captures.md)
