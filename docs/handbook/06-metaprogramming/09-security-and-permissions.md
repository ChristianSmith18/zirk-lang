# Security and capability requirements

Decorator expansion is a build-time supply-chain boundary. The current public
Syntax API does not expose filesystem, environment, network, or process effects
to decorator code. `.zkinit` has one finite `requires` list and no
`during: build` or `compile_permissions` grant. A capability declaration is not
consent to perform a host operation.

Every observed input becomes part of the expansion fingerprint. Changed
requester versions, dependency paths, or observations invalidate affected
cache nodes. Generated source returns through normal validation. Secrets must
not enter public generated structure, source, IR, diagnostics, logs, cache
keys, manifests, or lockfiles.

If a future compiler service adds a protected build-time effect, its operation
boundary and consent contract must be specified before exposing it.

---

**Previous:** [← Generated Diagnostics](08-generated-diagnostics.md) · **Next:** [Toolchain](../07-toolchain/README.md)
