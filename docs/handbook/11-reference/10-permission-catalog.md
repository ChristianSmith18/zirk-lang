# Permission Catalog

Capability families include scoped filesystem read/write, network origins,
process execution/arguments, separate shell execution, environment names,
secrets, native libraries, and signals/system information.

Libraries declare `requires`; applications grant `permissions`. Every operation
uses `during: build`, `runtime`, or `both`, including decorator access. `unsafe`
grants nothing. Secret names may be scoped, but values never belong in
declarations, manifests, lockfiles, diagnostics or history.

| Family | Scope examples | Enforcement notes |
|---|---|---|
| filesystem | canonical path patterns, read/write | rejects traversal, symlink escape and check/use races |
| network | protocol, host, port | redirects, DNS results and reconnects remain in scope |
| process | canonical executable, argument forms | `arguments: all` warns |
| shell | explicit high-risk grant | never implied by process permission |
| environment | exact names or explicit `all` | broad name listing requires broad read |
| secrets | exact names | returns redacted `SecretString` |

Declaration requires a separate signed approval bound to project name and
canonical location plus requester fingerprints. `all` requires reinforced
interactive confirmation.

---

**Previous:** [← Target Matrix](09-target-matrix.md) · **Next:** [ Standard Library Index](11-standard-library-index.md)
