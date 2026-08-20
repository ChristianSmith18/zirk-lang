# Build an HTTP Service

Build a loopback service with one typed route and an E2E health check.

> **Status:** target Zirk 1.x `std.http`; server/runtime delivery remains phased.

```text
permissions {
    network {
        listen: { origins: ["http://127.0.0.1:8080"]; during: runtime; }
    }
}
```

```zirk
import { HTTPMethod, HTTPRequest, HTTPResponse, HTTPServer } from std.http;
import { SocketAddress } from std.net;

fn health(request: HTTPRequest): Result<HTTPResponse, HTTPError> {
    return Ok(HTTPResponse.json({ "status": "ok" }));
}

fn main(): Void {
    mut server = HTTPServer(
        address: SocketAddress("127.0.0.1", 8080),
        request_body_limit: 1MiB,
        header_limit: 32KiB,
    );
    server.route(HTTPMethod.Get, "/health", health);
    await server.serve();
}
```

Route registration rejects duplicates and ambiguous shapes. Each handler runs
inside the server scope; disconnect cancels its work, streaming observes
backpressure, and shutdown uses a bounded grace period before canceling
remaining handlers. CPU-heavy transforms belong in `parallel`, not one native
thread per request.

Add validation/authentication as ordinary middleware or decorators. Keep body,
header, decompression, timeout, redirect, and connection limits explicit.
Translate domain errors to an integer-backed `HTTPStatus` in one exhaustive
function so handlers do not drift.

Unit-test `health` directly. E2E-test the loopback response, unsupported method,
malformed/b oversized requests, cancellation, graceful shutdown, missing
permission, and bounded concurrent load with a runner-allocated port. See
[`std.http`](../04-standard-library/12-std-http.md) and
[Runtime Permissions](../03-projects/07-runtime-permissions.md).

## Completion contract

- **Prerequisites:** tasks, resources, permissions, `std.net`, and `std.http`.
- **Expected success:** `GET /health` returns bounded JSON with status 200.
- **Failure recovery:** invalid configuration fails before serving; request
  errors become typed responses and shutdown awaits handler cleanup.
- **Next:** package reusable behavior as a [library](05-build-and-publish-a-library.md).

---

**Previous:** [← Build a Concurrent Worker Pool](03-build-a-concurrent-worker-pool.md) · **Next:** [ Build and Publish a Library](05-build-and-publish-a-library.md)
