# `std.time`

The module separates `Duration`, monotonic instants, civil date/time, versioned time zones, timers, and cancelable sleep. Elapsed measurement uses a monotonic clock; civil time is never mixed implicitly with a duration.

Timer and sleep operations suspend tasks and observe cancellation. Wall-clock changes cannot corrupt monotonic elapsed measurements.

---

**Previous:** [← std.collections](06-std-collections.md) · **Next:** [ std.task](08-std-task.md)
