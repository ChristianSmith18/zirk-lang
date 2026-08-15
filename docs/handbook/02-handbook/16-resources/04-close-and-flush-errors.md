# Close and Flush Errors

Closing or flushing can fail. The resource contract defines how that failure is exposed, especially when another error is already active.

A close failure must never silently erase the primary operation failure. APIs may aggregate, attach, log safely, or prioritize according to a documented typed rule.

Callers that require durability must distinguish “write accepted” from “flush and close succeeded.”

---

**Previous:** [← Resource Transfer](./03-resource-transfer.md) · **Next:** [Cancellation and Cleanup →](./05-cancellation-and-cleanup.md)
