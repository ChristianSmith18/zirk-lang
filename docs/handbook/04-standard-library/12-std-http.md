# `std.http`

The initial native client/server provides typed requests and responses, validated headers, streaming, backpressure, timeouts, configurable limits, audited TLS, and disconnect cancellation.

Handlers run as structured tasks; no thread is created per request. CPU-heavy handler work moves to `parallel`. Routing frameworks, ORM, and templates remain external packages.

---

**Previous:** [← `std.net`](./11-std-net.md) · **Next:** [`std.json` →](./13-std-json.md)
