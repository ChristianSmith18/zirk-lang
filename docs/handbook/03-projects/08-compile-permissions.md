# Permission phases removed

`.zkinit` has one finite `requires` block. There are no `permissions`,
`compile_permissions`, or `during: build | runtime | both` declarations.
Compiler-time and runtime consent are not split into separate manifest
sections.

When migrating an older project, `zirk migrate --check` previews how legacy
permission declarations become capability requirements. Applying the
migration does not create consent or broaden the user's saved approval.

See [Runtime consent](07-runtime-permissions.md) for how a concrete protected
operation obtains user approval.

---

**Previous:** [← Runtime consent](07-runtime-permissions.md) · **Next:** [Library requirements](09-library-requirements.md)
