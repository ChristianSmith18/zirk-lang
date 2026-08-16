# Library Requirements

Libraries declare `requires` for capabilities their public behavior needs. Applications decide whether to satisfy those requirements and may reject a dependency whose needs exceed policy.

Requirements should be narrow and explain which API needs them. Optional functionality must not inflate every consumer's required permission set.

Each request identifies scope and phase. A library requests but never grants.
The application approval records the exact requester name, version, integrity,
transitive path and phase; updating code that receives authority requires fresh
consent even if the textual permission scope is unchanged.

---

**Previous:** [← Compile Permissions](08-compile-permissions.md) · **Next:** [ Build Targets](10-build-targets.md)
