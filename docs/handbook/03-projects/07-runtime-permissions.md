# Application Permissions

Permissions are finite capabilities for filesystem, network, processes,
environment/secrets, and other effects. Only the application grants the final
set in `.zkinit`; each operation identifies `during: build`, `runtime`, or
`both`.

The full permission model can infer requirements through privileged APIs and
callable metadata; developers do not annotate each `Fn`. In the current native
`std.net` and `std.environment` preview, package `.zkinit` files declare these
requirements explicitly using a `permissions` block, while the application
approval command resolves the locked dependency graph and asks for consent to
the combined scopes. Deployed runtime never prompts or edits its manifest:
dynamic denial is `Error(PermissionDeniedError)`. Secret values never enter
manifests or lockfiles.

---

**Previous:** [← Globals](06-globals.md) · **Next:** [Build and Runtime Phases →](08-compile-permissions.md)
