# Dynamic Libraries

Dynamic loading is platform-sensitive and requires native/library permissions. The API must model lookup failure, version mismatch, missing symbols, architecture mismatch, and library lifetime.

Function pointers cannot outlive the loaded library. Prefer build-time linked dependencies when runtime selection is unnecessary.

Loading returns a managed library resource. Symbol lookup returns a typed
function handle whose lifetime depends on that resource; storing or returning
the function after unload is rejected. Closing is idempotent and waits for no
new calls, but callers must structure concurrent calls so unload cannot race
them.

Search never trusts ambient current-directory order. The API accepts an
explicit canonical path or a package-declared artifact selected for the target.
The permission check covers the library path and native-load operation.
Signatures, architecture, object format, ABI version, and optional integrity
metadata are validated before invocation.

Dynamic loading is appropriate for reviewed plugins or system components whose
runtime selection is required. Ordinary dependencies should remain locked,
verified, and linked at build time for reproducibility.

---

**Previous:** [← Exporting Zirk](03-exporting-zirk.md) · **Next:** [ Name Mangling](05-name-mangling.md)
