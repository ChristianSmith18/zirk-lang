# `.zkinit`

`.zkinit` is a declarative typed DSL, not executable startup code. The
implemented project header declares a logical `project` name, `kind
application` or `kind package`, optional `entry`, one or more `source` roots,
and dependencies. A dependency declares a logical name, version constraint and
one transport (`local`, `registry`, `git` or `vendored`); source imports never
mention that transport. A dependency-bearing project requires matching
`zirk.lock` metadata before frontend checking. It may also contain
`build_targets`, `globals`, `permissions`, `requires`, and dependencies
according to project type. A permission operation carries
`during: build`, `runtime`, or `both`; there is no separate
`compile_permissions` block.

It does not contain arbitrary runtime/compiler configuration, imports, tokens,
secrets, or proof of consent. Signed approval lives outside the repository and
is bound to project name and canonical location plus exact requesters. Invalid
fields receive manifest diagnostics before any privileged code executes.

```text
project "acme.cli"
kind package
entry "src/package.zrk"
source "src"
dependency acme.text ^1 local "../text"
```

---

**Previous:** [← Project Layout](01-project-layout.md) · **Next:** [ Applications](03-applications.md)
