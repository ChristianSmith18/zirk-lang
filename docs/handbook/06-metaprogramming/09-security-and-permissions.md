# Security and Permissions

Decorator execution is a build-time supply-chain boundary. Filesystem, network, and process access require finite `compile_permissions`; runtime permissions do not imply them.

Generated source returns through normal validation. Secrets must not enter generated diagnostics, cached IR, manifests, or lockfiles.

---

**Previous:** [← Generated Diagnostics](./08-generated-diagnostics.md) · **Next:** Toolchain *(next section)*
