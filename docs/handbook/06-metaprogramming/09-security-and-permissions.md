# Security and Permissions

Decorator argument evaluation and expansion are build-time supply-chain boundaries. Filesystem, environment, network, process, and other privileged effects require finite application grants marked `during: build` or `both`; runtime grants do not imply them. Libraries request authority but cannot grant it.

Every observed external value becomes part of the expansion fingerprint. Changed permissions, requester versions, dependency paths, or observations invalidate affected cache nodes. Approval remains signed outside the repository, bound to project location and requester integrity, and is checked before execution.

Generated source returns through normal validation. Secrets must not enter public generated structure, source, IR, diagnostics, logs, cache keys, manifests, or lockfiles. Missing or unverifiable authority fails closed.

---

**Previous:** [← Generated Diagnostics](08-generated-diagnostics.md) · **Next:** [ Toolchain](../07-toolchain/README.md)
