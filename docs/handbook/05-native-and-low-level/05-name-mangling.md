# Name Mangling

Zirk may mangle internal symbols to represent modules, types, and generics. Mangling is not a stable external API. C-exported entry points use explicit unmangled names governed by the ABI contract.

Package compatibility relies on typed public API and IR versions, not reverse engineering linker symbols.

Internal mangling may change between compiler versions and may encode module
identity, declaration kind, concrete generic specialization, receiver shape,
and private collision-resistant identity. Debug information maps the native
symbol back to its source declaration.

`extern "C"` imports name the symbol expected from the native library. Exports
must choose a unique stable symbol explicitly; the compiler diagnoses duplicate
exports across generated and handwritten declarations. Platform decoration
such as leading underscores is handled by the target linker layer, not written
into portable Zirk source.

Never use an internal mangled name in FFI, package metadata, reflection, or
documentation. Those surfaces use declared typed identity.

---

**Previous:** [← Dynamic Libraries](04-dynamic-libraries.md) · **Next:** [ Native Permissions](06-native-permissions.md)
