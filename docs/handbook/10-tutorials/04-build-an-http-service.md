# Build an HTTP Service

Create an application granting only the required network address. Use typed `std.http` requests, validated headers, bounded bodies, and a response enum that maps domain errors to status codes.

Handlers run as structured tasks and inherit disconnect cancellation. Stream large bodies with backpressure; move CPU-heavy work into `parallel` rather than creating a thread per request. Test handler functions directly and run permission-scoped E2E requests against an isolated port.

---

**Previous:** [← Build a Concurrent Worker Pool](./03-build-a-concurrent-worker-pool.md) · **Next:** [Build and Publish a Library →](./05-build-and-publish-a-library.md)
