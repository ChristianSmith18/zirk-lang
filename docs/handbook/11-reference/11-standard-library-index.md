# Standard Library Index

The 1.x module catalog includes `std.io`, `std.fs`, `std.path`, `std.process`, `std.net`,
`std.http`, `std.json`, `std.crypto`, `std.time`, `std.thread`, `std.sync`,
`std.collections`, `std.testing`, `std.reflect`, `std.system`, and
`std.environment` (`Environment`, preferred alias `Env`). This is the target
module catalog, not a claim that each name is an importable package. For
example, collections and synchronization are compiler-known language surfaces
without importable `std.collections` or `std.sync` packages. Structured task
operations and CPU parallelism are language surfaces, not importable
`std.task` or `std.parallel` modules.

See the [Standard Library](../04-standard-library/README.md) for module chapters
and the [Standard Library Indexes](../04-standard-library/19-standard-library-indexes.md)
to navigate by task, module, type, error, permission, waiting/cancellation, and
complexity.

---

**Previous:** [← Permission Catalog](10-permission-catalog.md) · **Next:** [ Feature Status](12-feature-status.md)
