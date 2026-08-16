# Security and Permissions

Decorator execution is a build-time supply-chain boundary. Filesystem, network,
and process access require finite application grants marked `during: build`;
runtime grants do not imply them. A dependency update requires fresh approval
before the new code receives existing authority.

Generated source returns through normal validation. Secrets must not enter generated diagnostics, cached IR, manifests, or lockfiles.

Repository source cannot approve itself. Consent is signed outside the project,
bound to its canonical location and exact requester, and checked before any
decorator executes.

---

**Previous:** [← Generated Diagnostics](08-generated-diagnostics.md) · **Next:** [ Toolchain](../07-toolchain/README.md)
