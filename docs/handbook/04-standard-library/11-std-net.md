# `std.net`

`std.net` provides typed addresses, DNS, TCP, and UDP APIs integrated with the reactor. Operations are cancelable and respect `permissions.network`.

Sockets are resources; connect, read, write, shutdown, DNS errors, timeouts, and peer closure have explicit typed contracts. APIs apply bounds and backpressure rather than buffering without limit.

---

**Previous:** [← `std.sync`](./10-std-sync.md) · **Next:** [`std.http` →](./12-std-http.md)
