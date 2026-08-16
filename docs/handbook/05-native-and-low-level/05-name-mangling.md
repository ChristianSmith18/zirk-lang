# Name Mangling

Zirk may mangle internal symbols to represent modules, types, and generics. Mangling is not a stable external API. C-exported entry points use explicit unmangled names governed by the ABI contract.

Package compatibility relies on typed public API and IR versions, not reverse engineering linker symbols.

---

**Previous:** [← Dynamic Libraries](04-dynamic-libraries.md) · **Next:** [ Native Permissions](06-native-permissions.md)
