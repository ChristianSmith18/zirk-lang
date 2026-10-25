# Projects

`.zkinit` defines whether a project is an application or package and declares
entry/root selection, targets, dependencies, globals, and capability
requirements. `zirk.lock` records exact package selections and source identity.

Applications and packages use one `requires` block. A library requests a
capability but does not grant it. User consent is separate and applies to the
concrete resource at the protected operation boundary. The runtime retains
requester and dependency-path provenance for each request.

`zirk init` creates a canonical `.zkinit` interactively or with `zirk init -y`
for the documented defaults. See the project chapters for configuration,
initialization, [migration](15-migrating-zkinit.md), and current runtime limitations.

---

**Previous:** [← Circular Dependencies](../02-handbook/19-modules/07-circular-dependencies.md) · **Next:** [Project Layout](01-project-layout.md)
