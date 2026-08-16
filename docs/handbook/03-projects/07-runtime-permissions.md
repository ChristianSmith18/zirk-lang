# Runtime Permissions

Permissions are finite capabilities for filesystem, network, processes, environment, and other effects. The application grants the final set in `init.zrk`.

Missing permission produces a clear error, never an interactive grant during `zirk build`. Tokens and secrets are not permissions and never belong in the manifest or lockfile.

---

**Previous:** [← Globals](06-globals.md) · **Next:** [ Compile Permissions](08-compile-permissions.md)
