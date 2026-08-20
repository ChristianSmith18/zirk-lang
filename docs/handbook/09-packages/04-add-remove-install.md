# Add, Remove, and Install

`zirk add` and `remove` modify manifest requirements; `zirk install` resolves and verifies them. Commands update the lockfile deterministically and explain conflicts, unavailable targets, permissions, and native requirements.

Review manifest and lockfile changes together.

```bash
zirk add example@^2.1
zirk remove example
zirk install
```

`add` resolves the requested constraint and shows new transitive packages,
requirements, native code, build permissions, target restrictions, and lock
changes before writing. `remove` removes dependencies no longer reachable but
does not silently change unrelated constraints. `install` follows the existing
manifest/lock state and verifies content; it does not perform an implicit
update.

CI is noninteractive and fails on missing lock entries, integrity mismatch, or
authority requiring approval.

---

**Previous:** [← Portable IR](03-portable-ir.md) · **Next:** [ Version Resolution](05-version-resolution.md)
