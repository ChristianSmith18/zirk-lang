# `init.zrk`

`init.zrk` is a declarative typed DSL, not executable startup code. It may contain `project`, `build_targets`, `globals`, `permissions`, `compile_permissions`, `requires`, and dependencies according to project type.

It does not contain arbitrary runtime/compiler configuration, imports, tokens, or secrets. Invalid fields receive manifest diagnostics before compilation.

---

**Previous:** [← Project Layout](01-project-layout.md) · **Next:** [ Applications](03-applications.md)
