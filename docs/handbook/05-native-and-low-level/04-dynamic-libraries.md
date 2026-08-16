# Dynamic Libraries

Dynamic loading is platform-sensitive and requires native/library permissions. The API must model lookup failure, version mismatch, missing symbols, architecture mismatch, and library lifetime.

Function pointers cannot outlive the loaded library. Prefer build-time linked dependencies when runtime selection is unnecessary.

---

**Previous:** [← Exporting Zirk](03-exporting-zirk.md) · **Next:** [ Name Mangling](05-name-mangling.md)
