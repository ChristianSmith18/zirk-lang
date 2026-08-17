# Pointers

Raw `Pointer<T>` values provide low-level memory access inside `unsafe` code and
may be null. They never implicitly become safe references.

```zirk
unsafe {
    if pointer.is_null {
        return Error(NativeError("null pointer"));
    }

    pointer.write(20);
    mut next = pointer + 1;
}
```

The programmer must uphold alignment, validity, lifetime, aliasing, and native ownership contracts. A pointer does not bypass permissions or operating-system protection.

Arithmetic counts `T` elements. Use `Pointer<Byte>` or `offset_bytes` for byte
offsets and `cast<T>()` for representation casts. Prefer bounded
`NativeSlice<T>` and `NativeSliceMut<T>` views: construction validates extent,
alignment and lifetime, and ordinary indexed access remains bounds checked.

---

**Previous:** [← Safe References](04-safe-references.md) · **Next:** [ Dereferencing](06-dereferencing.md)
