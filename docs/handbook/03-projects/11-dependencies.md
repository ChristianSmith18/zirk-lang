# Dependencies

A dependency entry is keyed by the package's logical import name. The common
form uses a version requirement from the configured default registry:

```text
dependencies {
    acme.http: "^2.1";
}
```

Use an expanded entry to select a source:

```text
dependencies {
    acme.shared: { from: "path:../shared"; }
    acme.crypto: { from: "git+https://code.example/acme/crypto"; rev: "v1.4.2"; }
    acme.private: { from: "registry:company"; version: "^3.0"; }
}

dev_dependencies {
    acme.testkit: "^0.8";
}
```

Sources include the configured or named registry, a project-relative local
path, Git with an immutable locked revision, and vendored source. The import is
always transport-independent:

```zirk
import { Client } from acme.http;
```

`dependencies` participate in build/run and downstream package resolution.
`dev_dependencies` are available only to this project's designated
development/test graph and are not inherited by consumers. `zirk.lock` records
selected versions, sources, revisions or integrity hashes, and package roots.
`zirk install` verifies the existing selection; `zirk update` is the explicit
command that refreshes eligible versions. Registry/Git acquisition remains a
partial implementation and is listed in the feature-status catalog.

---

**Previous:** [← Build targets](10-build-targets.md) · **Next:** [Lockfile](12-lockfile.md)
