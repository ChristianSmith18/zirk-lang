# Cancellation and Cleanup

Cancellation is cooperative and observed at safe points. Before a cancelled task finishes, its resource scopes close and the runtime waits for required cleanup.

Cleanup should be bounded and cancellation-aware without abandoning invariants. A second shutdown signal or exhausted limit may force controlled termination, so cleanup cannot assume unlimited time.

Provable abandonment, duplicate close, illegal escape, and use-after-transfer
are compile errors. Dynamic use of a closed or transferred resource produces a
typed runtime failure. Runtime leak reporting is defense-in-depth and never
counts as successful deterministic cleanup.

---

**Previous:** [← Close and Flush Errors](04-close-and-flush-errors.md) · **Next:** [ Why No General defer or Destructors?](06-why-no-defer-or-destructors.md)
