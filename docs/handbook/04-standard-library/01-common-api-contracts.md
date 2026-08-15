# Common API Contracts

Stdlib APIs prefer enums and options to magic strings, `Result` for expected operational failure, and exceptions for exceptional recoverable failure. Operations that may wait are cancelable and document blocking, thread safety, allocation, limits, resources, and permissions.

Filesystem APIs accept `Path`; hostile input receives size/depth limits. Supported targets behave equivalently or document a platform difference. Async variants suspend a task instead of occupying a thread.

---

**Previous:** [← Standard Library](./README.md) · **Next:** [`std.io` →](./02-std-io.md)
