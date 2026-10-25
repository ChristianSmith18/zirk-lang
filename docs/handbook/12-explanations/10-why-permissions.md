# Why capabilities?

Types do not reveal every filesystem, network, process, or environment effect.
Zirk uses one finite `requires` list to make the kinds of protected operations
visible in application and package manifests.

A requirement and user consent answer different questions. A package may need
to connect to a service, but only the user can approve a concrete host and port
for the consuming application. The runtime asks at the operation boundary,
keeps read/write and connect/listen separate, and denies only the unapproved
operation. Applications start without every possible resource already
approved; programs can handle a permission error like other `Result` outcomes.

The user's signed exact-resource consent lives outside the repository. The
manifest is not an approval file, and a dependency cannot grant itself access.
See [runtime requirements](../03-projects/07-runtime-permissions.md) and
[permission audit](../03-projects/14-permission-approval-and-audit.md).

---

**Previous:** [← Why the C ABI?](09-why-c-abi.md) · **Next:** [Why Portable IR?](11-why-portable-ir.md)
