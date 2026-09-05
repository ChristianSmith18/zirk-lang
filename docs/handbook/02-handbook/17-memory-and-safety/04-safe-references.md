# Safe References

Safe references are non-null and cannot outlive the value or resource on which
they depend. The compiler tracks escape and alias constraints internally without
exposing Rust-style lifetime syntax in Zirk 1.x.

## Resource-derived references

A reference derived inside `match with` cannot escape after the resource closes.
Sharing mutable referenced state across concurrent contexts requires
synchronization.

```zirk
match with File.open("data.txt") as file {
    line => stdout.println(line),
}
// the file is closed; no handle may escape
```

## Weak references

`Weak<T>` observes an object without keeping the referent alive. It is useful for
cache entries, listeners, and other non-owning relationships.

```zirk
mut cache = "expensive";
mut weak = Weak.from(cache);

match weak.upgrade() {
    null => stdout.println("gone"),
    live => stdout.println(live),
}
```

`upgrade(): T?` is required before use. `is_alive` is only an immediate
observation and cannot replace upgrading.

```zirk
if weak.is_alive {
    // the object may die before the next line
    weak.upgrade() // always do this
}
```

## Dependent references

`Dependent<T>` represents a view whose validity is tied to a larger owner. The
compiler rejects any escape beyond the owner's lifetime.

```zirk
mut owner = List(1, 2, 3);
mut view = owner.as_view(); // Dependent<List<Int32>> or similar

// view cannot be returned or stored where owner is not also alive
```

## Pinning

Some storage must not move while it is being accessed by native code or another
dependent view. `Pin<T>` is a marker that the object is pinned in place for the
duration of the dependent reference.

```zirk
unsafe {
    mut pinned = Pin.from(buffer);
    mut slice = NativeSlice<Byte>(base: pinned.address(), length: 64);
    // buffer cannot be moved or reallocated while slice is alive
}
```

Zirk 1.x does not expose a general `Pin<T>` lifetime annotation; pinning is
coordinated through `unsafe` and native-view contracts.

## When to choose what

- Use a normal managed reference (`String`, `Array<T>`, `List<T>`, `class`) when
  you want shared ownership with the GC.
- Use `Weak<T>` when you observe an object you do not want to keep alive.
- Use `Dependent<T>` when the view must not outlive the owner.
- Use `Pin<T>` when native or dependent access requires a stable address.

## API

### `Weak<T>`

A non-owning managed reference that observes an object without keeping it
alive. Delivered: `Weak.from`, `.upgrade(): T?`, `.is_alive`, cleared before
the referent is freed.

#### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `is_alive` | `Boolean` | Immediate observation only — cannot replace upgrading | implemented |

#### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Weak.from(referent)` | `Weak<T>` | Static construction | implemented |
| `w.upgrade()` | `T?` | Acquires a strong reference or `null` | implemented |
| `w.to_string()` | `String` | Universal member | implemented |

#### Examples

```zirk
mut cache = "expensive";
mut weak = Weak.from(cache);

match weak.upgrade() {
    null => stdout.println("gone"),
    live => stdout.println(live),
}
```

### `Dependent<T>`

A view whose validity is tied to a larger owner; the compiler rejects any
escape beyond the owner's lifetime. Type and IR support exist; automatic
lifetime integration is active work — members are `partial`/`specified`.

#### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `owner.as_view()` | `Dependent<Owner>` | Derives a dependent view (owner-provided) | partial |
| `d.get()` | `T` | Accesses the viewed value | specified — invented; no accessor is documented |

#### Examples

```zirk
mut owner = List(1, 2, 3);
mut view = owner.as_view();      // Dependent<List<Int32>> or similar
// view cannot be returned or stored where owner is not also alive
```

### `Pin<T>`

Marker that an object must not move while native code or a dependent view
accesses it. No general `Pin<T>` lifetime annotation in Zirk 1.x; pinning is
coordinated through `unsafe` and native-view contracts. Members are `partial`/
`specified`.

#### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Pin.from(referent)` | `Pin<T>` | Static pinning (`unsafe` context) | partial |
| `p.address()` | `Pointer<T>` | Stable address for native view construction | partial — used in the safe-references example |
| `p.get()` | `T` | Accesses the pinned value | specified — invented; no accessor is documented |

#### Examples

```zirk
unsafe {
    mut pinned = Pin.from(buffer);
    mut slice = NativeSlice<Byte>(base: pinned.address(), length: 64);
    // buffer cannot be moved or reallocated while slice is alive
}
```

## Implementation status

> `Weak<T>` and basic safe-reference tracking are delivered. `Dependent<T>` and
> `Pin<T>` support exists at the type and IR level but automatic pinning and
> full lifetime integration remain active work.

---

**Previous:** [← Automatic Memory Management](03-automatic-memory-management.md) · **Next:** [ Pointers and Native Slices](05-pointers.md)
