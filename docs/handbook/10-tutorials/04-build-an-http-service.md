# Build an HTTP Service

Build a bounded loopback HTTP/1.1 service with one typed route and a JSON
response. The current server API serves a configured finite number of requests
and then releases its listener.

```text
permissions {
    network {
        listen: { addresses: ["127.0.0.1:8080"]; during: runtime; }
    }
}
```

```zirk
import { HTTPError, HTTPMethod, HTTPRequest, HTTPResponse, HTTPRouteParameters, HTTPRouter, HTTPServer } from std.http;
import { stderr, stdout } from std.io;

fn health(request: HTTPRequest, params: HTTPRouteParameters): HTTPResponse {
    return HTTPResponse.from_json(200, { value: 42 });
}

fn main(): Void {
    match HTTPServer.bind("tcp://127.0.0.1:8080") {
        Ok(listener) => {
            mut router: HTTPRouter = HTTPRouter();
            match router.route(HTTPMethod.Get, "/health", health) {
                Ok(_) => match router.serve_and_close(listener, 1) {
                    Ok(count) => stdout.println(count);
                    Error(error) => stderr.println(error);
                };
                Error(error) => stderr.println(error);
            };
        }
        Error(error) => stderr.println(error);
    }
}
```

`HTTPServer.bind` checks the exact listen permission before opening the socket.
`HTTPRouter.route` validates route patterns and rejects duplicate or ambiguous
routes. The server preview uses a finite request bound; this example accepts
one request, sends its response, and closes the listener. For a longer-lived
service, choose an explicit request bound and supervise the serving task.

Route handlers return `HTTPResponse`. `HTTPResponse.from_json` sets the JSON
content type; its argument is already serialized JSON text. Use
`HTTPResponse.from_stream` when a response should be produced incrementally.
The producer runs on its owning Zirk task, and the transport applies bounded
backpressure while sending chunks.

Exercise the service by starting it and sending one request to
`http://127.0.0.1:8080/health`; the response status is 200 and its JSON body is
`42`. The server and route APIs are a bounded preview; consult
[`std.http`](../04-standard-library/12-std-http.md) for current protocol and
lifecycle limitations.

## Completion contract

- **Prerequisites:** a Zirk compiler with the bundled `std.net` and `std.http`
  preview APIs.
- **Expected success:** one `GET /health` returns JSON with status 200, after
  which the finite server scope exits and releases its listener.
- **Failure recovery:** bind, route, and serving errors are returned as typed
  results; correct the address, permission, or route before retrying.
- **Next:** package reusable behavior as a
  [library](05-build-and-publish-a-library.md).

---

**Previous:** [← Build a Concurrent Worker Pool](03-build-a-concurrent-worker-pool.md) · **Next:** [Build and Publish a Library](05-build-and-publish-a-library.md)
