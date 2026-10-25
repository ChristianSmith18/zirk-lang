# Migrating project configuration

Use the migration preview before changing a legacy project:

```bash
zirk migrate --check
zirk migrate
```

`--check` reports the canonical result without writing. Applying a migration
preserves the original manifest as a recoverable backup, validates the new
manifest, and writes it atomically. It does not rewrite source files.

## Permission declarations

Legacy `permissions`, `compile_permissions`, `test_permissions`, and `during`
entries become capability names in the single `requires` block where the
capability is recognized. The migrated declaration describes possible code
behavior only. It does not create, copy, or widen user consent. The next
protected operation checks its exact resource against the user's signed
approval store and prompts when appropriate.

## Package entry and dependencies

A legacy package `entry` becomes `project.root`, because a package root is
importable and is not an application startup entry. Applications keep
`project.entry`. Legacy dependency declarations are normalized to the
name-keyed `dependencies`/`dev_dependencies` structure when their source
identity can be determined. The lockfile remains the authority for the exact
resolved version, revision/integrity, and package root; inspect the preview for
unresolved or unsupported sources before applying.

## Source-level `fn main`

A source `fn main` is no longer invoked implicitly. It remains an ordinary
function. The migration reports legacy entry files that rely on it but does
not move the function body or rewrite its callers. Move the statements that
should start the application to file scope, or call the function explicitly
from the selected `project.entry` file.

## After migration

Run `zirk check` and inspect `.zkinit` and `zirk.lock`. Run `zirk permissions
inspect .` and `zirk permissions diff .` to see requirements and exact saved
consent separately. A migration is not a security approval.

---

**Previous:** [← Permission approval and audit](14-permission-approval-and-audit.md) · **Next:** [Standard library](../04-standard-library/README.md)
