# Permission catalog

The manifest exposes one finite capability vocabulary:

- `filesystem.read`, `filesystem.write`
- `network.connect`, `network.listen`
- `network.udp.send`, `network.udp.listen`
- `network.local.connect`, `network.local.listen`
- `network.inspect_interfaces`
- `environment.read`, `environment.list_names`, `secrets.read`
- `process.execute`, `process.shell`

Applications and packages declare capabilities in the same `requires` block.
A requirement is not consent and cannot authorize a concrete resource. At the
protected operation, the runtime checks the exact resource and the user-owned
signed approval record. Read/write and connect/listen remain separate.

The current native runtime enforces environment/secret and network APIs,
including HTTP destinations, proxies, DNS answers, redirects, TCP/UDP
listeners, UDP destinations, local endpoints, and interface inspection.
Filesystem and process identifiers are reserved in the manifest model because
this checkout does not yet expose compiler-owned APIs for them. `unsafe` grants
nothing. Secret values never belong in manifests, lockfiles, diagnostics, or
history.

```text
zirk permissions inspect .
zirk permissions diff .
zirk permissions history .
zirk permissions revoke .
```

First-use consent occurs at the operation with an exact `y/n` prompt. There is
no blanket `approve` action or `all` scope.

---

**Previous:** [← Target Matrix](09-target-matrix.md) · **Next:** [Standard Library Index](11-standard-library-index.md)
