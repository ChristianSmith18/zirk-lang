# Packages

Zirk's project model separates a package's stable logical name from its
registry, Git, local, or vendored source. `zirk.lock` records exact source and
integrity metadata. Applications and packages declare capability needs in the
same `requires` block; signed user consent remains outside the package and
lockfile and applies to exact resources at runtime.

Local/vendored graph traversal, logical imports, and lock metadata validation
are implemented. Registry/Git acquisition, verified remote archives, and
publication remain partial; check the
[feature status catalog](../../init/ZIRK_FEATURE_STATUS.md) before relying on
the broader package lifecycle contract.

---

**Previous:** [← Benchmarks](../08-testing/07-benchmarks.md) · **Next:** [Package Anatomy](01-package-anatomy.md)
