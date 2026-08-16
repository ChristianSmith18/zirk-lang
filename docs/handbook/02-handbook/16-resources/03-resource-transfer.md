# Resource Transfer

A resource cannot escape a `match with` scope directly or indirectly through a collection, object, closure, or task.

Transfer requires `TransferableResource` and is explicit:

```zirk
inmut moved = file.transfer();
file.read_all(); // error: responsibility transferred
```

The receiver must close, re-manage, or transfer the resource. A dependent
resource cannot outlive its parent. Transfer to another process/runtime is a
distinct fallible API; task/thread transfer additionally requires the accepted
safe transfer contracts.

Prefer returning processed data rather than a live handle when callers do not need ownership.

A non-cloneable resource inside a collection is extracted by a moving operation
such as `take(index)`, not ordinary projection-copy indexing.

---

**Previous:** [← match with](02-match-with.md) · **Next:** [ Close and Flush Errors](04-close-and-flush-errors.md)
