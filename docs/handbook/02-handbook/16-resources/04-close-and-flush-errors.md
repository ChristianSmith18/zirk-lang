# Close and Flush Errors

Closing or flushing can fail. The resource contract defines how that failure is exposed, especially when another error is already active.

A close failure never erases the primary outcome:

```zirk
enum ResourceFailure<BodyError, CloseError> {
    Body(BodyError);
    Close(CloseError);
    BodyAndClose(body: BodyError, close: CloseError);
}
```

Body success plus close failure produces `Close`; a body `Result.Error` plus
close failure produces `BodyAndClose`. During exception propagation, the
throwable remains primary and the close error is appended to `suppressed`.

Callers that require durability must distinguish “write accepted” from “flush and close succeeded.”

---

**Previous:** [← Resource Transfer](03-resource-transfer.md) · **Next:** [ Cancellation and Cleanup](05-cancellation-and-cleanup.md)
