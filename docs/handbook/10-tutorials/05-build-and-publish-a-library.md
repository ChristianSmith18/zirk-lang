# Build and Publish a Library

Declare `type: library`, publish the smallest API with `share`, and keep implementation declarations private. Libraries declare `requires`; they cannot define globals or grant final permissions.

Document types, errors, cancellation, thread safety, and target constraints. Add unit tests, run `zirk prepare`, review typed public API and portable IR metadata, verify license and README, then create `.zpkg` and publish an immutable semantic version.

---

**Previous:** [← Build an HTTP Service](04-build-an-http-service.md) · **Next:** [ Call a C Library](06-call-a-c-library.md)
