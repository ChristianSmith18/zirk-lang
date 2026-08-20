# Packages

Zirk packages distribute typed public API, portable IR, manifest metadata, documentation, and licensing in `.zpkg`. Resolution is locked and verified; final code generation aligns every package with the application target and ABI.

A package is not a preauthorized executable bundle. Libraries declare
`requires`, applications grant `permissions`, and signed consent remains
external to both package and lockfile. Native artifacts and build-time code are
visible supply-chain boundaries reviewed by `zirk prepare`.

Read this unit in order when publishing. Consumers normally use `zirk add` and
commit both manifest and lockfile changes after reviewing version, integrity,
requirements, targets, and requester paths.

---

**Previous:** [← Benchmarks](../08-testing/07-benchmarks.md) · **Next:** [ Package Anatomy](01-package-anatomy.md)
