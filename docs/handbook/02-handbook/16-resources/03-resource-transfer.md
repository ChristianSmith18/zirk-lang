# Resource Transfer

A resource cannot escape a `match with` scope directly or indirectly through a collection, object, closure, or task.

Transfer requires an API contract that moves responsibility to a new owner before the original scope closes. The compiler must reject ambiguous double ownership and use-after-close.

Prefer returning processed data rather than a live handle when callers do not need ownership.

---

**Previous:** [← match with](02-match-with.md) · **Next:** [ Close and Flush Errors](04-close-and-flush-errors.md)
