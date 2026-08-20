# Native Dependencies

Native artifacts declare operating system, architecture, ABI, linkage, license, and required permissions. A target build fails early when any transitive native dependency is incompatible.

Prefer portable Zirk IR where possible; isolate native calls behind safe C ABI wrappers.

Metadata identifies each artifact per target, its integrity/source, static or
dynamic linkage, exported ABI/version, system SDK requirements, and whether a
build step produces it. Transitive native dependencies appear in preparation
and permission audit output.

The final application must choose one compatible artifact for every selected
target. Missing architecture, incompatible CRT/stdlib, duplicate symbol,
unknown license, or unverifiable binary fails before opaque linkage. Runtime
dynamic loading remains a separate permission-checked operation.

---

**Previous:** [← Publishing](07-publishing.md) · **Next:** [ Package Security](09-package-security.md)
