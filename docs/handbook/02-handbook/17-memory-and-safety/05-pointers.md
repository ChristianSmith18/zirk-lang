# Pointers

Raw `Pointer<T>` values provide low-level memory access inside `unsafe` code.

```zirk
unsafe {
    mut pointer: Pointer<Int32> = &value;
    *pointer = 20;
}
```

The programmer must uphold alignment, validity, lifetime, aliasing, and native ownership contracts. A pointer does not bypass permissions or operating-system protection.

---

**Previous:** [← Safe References](04-safe-references.md) · **Next:** [ Dereferencing](06-dereferencing.md)
