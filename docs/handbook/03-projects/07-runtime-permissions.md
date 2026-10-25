# Capability requirements and runtime consent

Applications and packages use one finite `requires` block to declare which
protected operation families their code may attempt:

```text
requires {
    environment.read;
    network.connect;
}
```

Requirements propagate through the locked dependency graph. Zirk retains each
requester's package identity, version/integrity, and full dependency path, even
when multiple requesters use the same effective scope. A package requests a
capability; it cannot authorize itself or the application. The application
does not need to copy transitive requirements by hand.

A declaration is not consent. At the protected operation, Zirk checks the
concrete resource, such as an environment variable name, network origin, or
listener address and port. Connecting does not grant listening, and read does
not grant write. A dynamically selected listener requests that exact endpoint;
port `0` is treated as an ephemeral-bind request for the selected address.

If a matching user-owned approval is missing and the CLI has a terminal, Zirk
shows the project, operation, capability, exact resource, and requesting
package/path, then asks `[y/N]`. Yes stores that exact scope and permits the
current operation. For a direct application request, the CLI ensures the
application `.zkinit` exists and adds a missing `requires` declaration. A
transitive dependency's requirement is not copied into the app manually. No
denies only that operation and writes no approval. Without a terminal, Zirk
fails closed without waiting for input.

Missing consent does not stop application startup. A preserved `Result` can be
handled with `match`; in an ordinary value context, the language's existing
error projection produces `null`. Zirk prints a deduplicated warning that the
operation was attempted and denied, with its resource and requester, without
printing secret values. Other program execution continues unless the program
chooses to stop.

The current native runtime enforces environment/secret and network operations,
including HTTP destinations, proxy origins, DNS answers, redirects, TCP/UDP
listeners, UDP destinations, local endpoints, and interface inspection.
Filesystem and process capability identifiers are reserved in the manifest
model, but this checkout does not yet expose compiler-owned filesystem or
process APIs to enforce them.

---

**Previous:** [← Globals](06-globals.md) · **Next:** [Permission phases](08-compile-permissions.md)
