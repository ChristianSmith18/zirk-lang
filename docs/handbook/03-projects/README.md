# Projects

`init.zrk` defines whether a project is an application or library and declares entry points, capabilities, targets, dependencies, and build contracts. `zirk.lock` records exact resolved versions and hashes.

Libraries request authority with `requires`; applications grant it with
`permissions`, including build/runtime phase. Source declaration is not
consent: signed approval is tied to the project name and canonical location and
to exact requesting dependencies, then checked incrementally before privileged
code executes.

---

**Previous:** [← Circular Dependencies](../02-handbook/19-modules/07-circular-dependencies.md) · **Next:** [ Project Layout](01-project-layout.md)
