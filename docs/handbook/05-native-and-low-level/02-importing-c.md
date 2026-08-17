# Importing C

Imported C declarations translate native types into explicit Zirk contracts. Nullable pointers, buffers, lengths, callbacks, ownership, thread safety, and error codes must be represented rather than inferred.

Keep the unsafe binding layer small and expose a safe wrapper returning Zirk values, `Result`, or managed resources. Target checks reject a library unavailable for the selected architecture.

Prefer `NativeSlice<T>`/`NativeSliceMut<T>` after validating pointer, length,
alignment, provenance, and owner lifetime. A known bounded mutable view can
participate in unsafe rollback. Unknown-effect native calls, native release, or
unbounded writes belong inside `commit`, because Zirk cannot reconstruct their
external state.

---

**Previous:** [← C ABI](01-c-abi.md) · **Next:** [ Exporting Zirk](03-exporting-zirk.md)
