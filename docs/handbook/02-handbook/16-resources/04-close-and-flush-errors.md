# Close and Flush Errors

Closing or flushing can fail. The resource contract defines how that failure is exposed, especially when another error is already active.

> **Implementation status:** `ResourceFailure<BodyError, CloseError>` is
> delivered for result-based body/close composition. Automatic suppression of
> a close failure while another exception is propagating remains specified
> work; `Throwable.suppressed()` itself is delivered for nested throws in a
> `catch`. See the [feature status catalog](../../../init/ZIRK_FEATURE_STATUS.md).

A close failure never erases the primary outcome:

```zirk
enum ResourceFailure<BodyError, CloseError> {
    Body(BodyError);
    Close(CloseError);
    BodyAndClose(body: BodyError, close: CloseError);
}
```

Body success plus close failure produces `Close`; a body `Result.Error` plus
close failure produces `BodyAndClose`. The behavior when a throwable is
already propagating remains part of the target contract.

Callers that require durability must distinguish “write accepted” from “flush and close succeeded.”

---

**Previous:** [← Resource Transfer](03-resource-transfer.md) · **Next:** [ Cancellation and Cleanup](05-cancellation-and-cleanup.md)
