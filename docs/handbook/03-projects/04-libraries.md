# Package libraries

A library is a package project. Its `project.root` selects its importable public
root (default `src/package.zrk`); it is not an application entry and is not
executed as the application. Consumers import its stable logical name rather
than the checkout or download path.

```text
project {
    name: "acme.http";
    version: "2.1.0";
    type: package;
    authors: ["Acme Engineering"];
    description: "HTTP client and server primitives.";
    license: "Apache-2.0";
    root: "src/package.zrk";
}

requires {
    network.connect;
}
```

A package can declare normal and development dependencies and its own
capability requirements. It cannot declare application globals or grant
consent. Transitive requirements retain their requester identity and dependency
path in the consuming application's requirement tree.

Distributed `.zpkg` publication and remote registry/Git acquisition remain
separate package-manager delivery limits; see the
[package status catalog](../../init/ZIRK_FEATURE_STATUS.md).

---

**Previous:** [← Applications](03-applications.md) · **Next:** [Project entry](05-entry-point.md)
