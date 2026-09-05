# Pointers and Native Slices

Raw `Pointer<T>` values provide low-level memory access inside `unsafe` code.
They never implicitly become safe references and the compiler does not check
alignment, lifetime, or aliasing for them.

## Pointers

A `Pointer<T>` is a raw address. It may be null, it may dangle, and it carries
no runtime ownership.

```zirk
unsafe {
    mut raw: Pointer<Int32> = some_native_address();

    if raw.is_null {
        return Error(NativeError("null pointer"));
    }

    *raw = 42;
    mut next = raw + 1;     // advances by one T
}
```

The programmer must prove alignment, validity, lifetime, and aliasing. A pointer
does not bypass permissions or operating-system protection.

### Pointer arithmetic

Arithmetic counts `T` elements. Use `Pointer<Byte>` or `offset_bytes` for byte
offsets and `cast<T>()` for representation casts:

```zirk
unsafe {
    mut bytes: Pointer<Byte> = some_buffer();
    mut as_ints = bytes.cast<Int32>();
    mut fifth = bytes + 20; // byte offset
}
```

### Null

A `Pointer<T>` may be `null`. Dereferencing `null` is undefined behavior, even
inside `unsafe`. Always check `is_null` first.

```zirk
unsafe {
    mut maybe: Pointer<Int32> = null;
    maybe.write(10); // undefined behavior, not a controlled error
}
```

## Native slices

`NativeSlice<T>` and `NativeSliceMut<T>` are bounded views into native or
managed storage. Construction validates extent, alignment, and lifetime, and
ordinary indexed access remains bounds checked. They are the safe alternative to
raw pointer arithmetic.

```zirk
unsafe {
    mut buffer: Pointer<Byte> = native_buffer();
    mut slice = NativeSlice<Byte>(base: buffer, length: 64);

    slice[0] = 0xFF;
    slice[64]; // error: index out of bounds
}
```

A `NativeSlice` is a dependent view: it borrows storage from a larger owner. It
cannot outlive that owner:

```zirk
mut slice: NativeSlice<Byte>;
{
    mut local = List(1, 2, 3);
    slice = local.as_bytes(); // error: slice cannot escape the local's lifetime
}
```

## When to choose what

- Use `Pointer<T>` only for native interop or when the compiler cannot prove
  safety.
- Use `NativeSlice<T>` when you need a bounded, typed view into native or
  managed memory.
- Use `Array<T>` or `List<T>` for ordinary, safe, managed collections.

## API

### `Pointer<T>`

A raw address usable only inside `unsafe`. May be null, may dangle, carries no
ownership; alignment, validity, lifetime, and aliasing are the programmer's
proof obligation. Dereferencing `null` is undefined behavior, not a controlled
error.

#### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `is_null` | `Boolean` | Null-address test | implemented |

#### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `p.read()` | `T` | Dereference read | implemented |
| `p.write(value)` | `Void` | Dereference write | implemented |
| `*p` / `*p = v` | `T` / `Void` | Dereference operators | implemented |
| `p + n` / `p - n` | `Pointer<T>` | Element-stride arithmetic | implemented |
| `p.offset(n)` | `Pointer<T>` | Named element-stride offset | implemented |
| `p.offset_bytes(n)` | `Pointer<Byte>` | Byte-granularity offset | implemented |
| `p.cast<U>()` | `Pointer<U>` | Representation cast | implemented |
| `p.as_slice(length)` | `Result<NativeSlice<T>, NativeError>` | Validated bounded view (`unsafe`) | implemented |
| `p.as_slice_mut(length)` | `Result<NativeSliceMut<T>, NativeError>` | Validated mutable view (`unsafe`) | implemented |
| `p.read_volatile()` / `p.write_volatile(v)` | `T` / `Void` | Volatile access for memory-mapped I/O | specified — named in the member index, signature invented |
| `p.address()` | `UInt64` | Numeric address for interop | specified — implied by `Pin` example usage |

#### Examples

```zirk
unsafe {
    mut raw: Pointer<Int32> = some_native_address();
    if raw.is_null {
        return Error(NativeError("null pointer"));
    }
    *raw = 42;
    mut next = raw + 1;          // advances by one Int32

    mut bytes: Pointer<Byte> = some_buffer();
    mut as_ints = bytes.cast<Int32>();
}
```

### `NativeSlice<T>` / `NativeSliceMut<T>`

Bounded dependent views into native or managed storage. Construction validates
extent, alignment, and lifetime; indexed access stays bounds checked, including
in safe code. A slice cannot outlive its owner.

#### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `Int32` | Element count | implemented |
| `is_empty` | `Boolean` | `length == 0` | implemented |

#### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `NativeSlice<T>(base:, length:)` | `NativeSlice<T>` | Validated construction inside `unsafe` | implemented |
| `s[i]` | `T` | Bounds-checked read | implemented |
| `s[i] = v` (`NativeSliceMut` only) | `Void` | Bounds-checked write | implemented |
| `s.iterator()` | `Iterator<T>` | Bounded iteration | specified |
| `s.subslice(start, end)` | `NativeSlice<T>` | Re-windowed view | specified — invented; no sub-view API documented |

Escape past the owner's lifetime is a compile-time error, not a runtime check.

#### Examples

```zirk
unsafe {
    mut buffer: Pointer<Byte> = native_buffer();
    mut slice = NativeSlice<Byte>(base: buffer, length: 64);
    slice[0] = 0xFF;
    slice[64];                   // error: index out of bounds
}
```

## Implementation status

> `Pointer<T>` and basic `unsafe` blocks are delivered. `NativeSlice<T>` and
> `NativeSliceMut<T>` are accepted for Phase 4e but may not yet cover all
> lifetime, pinning, and mutability combinations. Check the current feature
> status for the exact compiler support.

---

**Previous:** [← Safe References](04-safe-references.md) · **Next:** [ Dereferencing](06-dereferencing.md)
