# `init.zrk`

`init.zrk` is a declarative typed DSL, not executable startup code. It may
contain `project`, `build_targets`, `globals`, `permissions`, `requires`, and
dependencies according to project type. A permission operation carries
`during: build`, `runtime`, or `both`; there is no separate
`compile_permissions` block.

It does not contain arbitrary runtime/compiler configuration, imports, tokens,
secrets, or proof of consent. Signed approval lives outside the repository and
is bound to project name and canonical location plus exact requesters. Invalid
fields receive manifest diagnostics before any privileged code executes.

---

**Previous:** [← Project Layout](01-project-layout.md) · **Next:** [ Applications](03-applications.md)
