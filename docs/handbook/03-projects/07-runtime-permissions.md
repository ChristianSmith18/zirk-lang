# Application Permissions

Permissions are finite capabilities for filesystem, network, processes,
environment/secrets, and other effects. Only the application grants the final
set in `.zkinit`; each operation identifies `during: build`, `runtime`, or
`both`.

The compiler infers requirements through privileged APIs and callable metadata;
developers do not annotate each `Fn`. In a trusted interactive command, Zirk
may show the exact call/dependency path and offer to add and approve a narrow
grant. Deployed runtime never prompts or edits its manifest: dynamic denial is
`Error(PermissionDeniedError)`. Secret values never enter manifests or lockfiles.

---

**Previous:** [← Globals](06-globals.md) · **Next:** [Build and Runtime Phases →](08-compile-permissions.md)
