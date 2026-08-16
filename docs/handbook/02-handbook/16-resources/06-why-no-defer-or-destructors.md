# Why No General `defer` or Destructors?

General `defer` is excluded from Zirk 1.x, and destructors are not the public resource model. Both can obscure when typed acquisition, transfer, cancellation, and close errors occur.

`match with` makes the resource boundary visible and gives the compiler one construct whose exit paths it can verify. `finally` remains available for local exceptional cleanup, but it does not replace `Resource<E>`.

---

**Previous:** [← Cancellation and Cleanup](05-cancellation-and-cleanup.md) · **Next:** [ Memory and Safety](../17-memory-and-safety/README.md)
