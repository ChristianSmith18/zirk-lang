# Library requirements

Packages declare capability needs in the same finite `requires` block as
applications:

```text
requires {
    network.connect;
}
```

The compiler infers protected effects through direct and indirect calls,
closures, callbacks, and package boundaries. The manifest declaration is the
package's statement of possible behavior; it does not grant permission. At
runtime, the application user's consent is checked for the concrete resource.

The consuming application's internal requirement tree preserves the original
requesting package and all paths through which it is used. Equivalent exact
scopes may share a saved consent decision, while inspection still reports each
requester. The application does not need to duplicate a dependency's
requirement in its own `.zkinit`.

Optional library features should be structured so consumers can use
unprivileged APIs without invoking protected operations. A missing grant
denies only the attempted operation and is returned through the API's normal
error contract.

---

**Previous:** [← Permission phases](08-compile-permissions.md) · **Next:** [Build targets](10-build-targets.md)
