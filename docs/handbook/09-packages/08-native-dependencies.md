# Native Dependencies

Native artifacts declare operating system, architecture, ABI, linkage, license, and required permissions. A target build fails early when any transitive native dependency is incompatible.

Prefer portable Zirk IR where possible; isolate native calls behind safe C ABI wrappers.

---

**Previous:** [← Publishing](./07-publishing.md) · **Next:** [Package Security →](./09-package-security.md)
