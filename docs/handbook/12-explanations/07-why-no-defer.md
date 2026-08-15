# Why No General `defer`?

General `defer` can scatter cleanup obligations and obscure typed close failure, transfer, and cancellation. Zirk 1.x instead uses `Resource<E>` with `match with` and uses `finally` for local exceptional cleanup.

The exclusion narrows the language while strengthening one auditable resource path.

---

**Previous:** [← Why No Traditional Overloading?](./06-why-no-traditional-overloading.md) · **Next:** [Why No Inline Assembly? →](./08-why-no-inline-assembly.md)
