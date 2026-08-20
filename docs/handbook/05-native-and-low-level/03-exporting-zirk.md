# Exporting Zirk

An exported function exposes a stable C-compatible symbol and signature. It cannot leak Zirk-specific generics, exceptions, object layout, or automatic-memory internals across the boundary.

Catch exceptions before crossing into C, translate failure to the declared ABI, and document who owns every returned buffer or handle.

An exported declaration uses an explicit C symbol and only C-compatible types.
The compiler rejects exported overload-like constructor sets, generic
specializations without a concrete wrapper, Zirk object references, and a
`throws` effect that could cross the boundary.

```zirk
export extern "C" fn zirk_checksum(
    bytes: NativeSlice<UInt8>,
    output: Pointer<UInt64>,
): CStatus unsafe {
    // Validate pointers and translate every outcome to CStatus.
}
```

Callbacks entering Zirk must establish the required runtime context, validate
thread entry, catch all exceptions, and obey cancellation/reentrancy rules.
Returned allocations use an exported matching release function or caller-owned
buffer; ownership may never depend on knowledge of Zirk's internal allocator.

Exported ABI tests load the produced symbol from C, exercise success and every
declared failure, and verify that no pending exception or managed reference
escapes.

---

**Previous:** [← Importing C](02-importing-c.md) · **Next:** [ Dynamic Libraries](04-dynamic-libraries.md)
