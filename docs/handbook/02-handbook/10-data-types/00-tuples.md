# Tuples

A tuple is a fixed-size heterogeneous value. Its type uses `Tuple(...)`; its
literal uses parentheses. Elements are accessed with the same `[N]` syntax as
arrays and lists, but the index must be a compile-time integer literal so the
checker knows the result type.

```zirk
inmut result: Tuple(String, Int, Bool) = ("Ada", 37, true)
inmut name = result[0]
inmut active = result[2]
```

Tuples are not sequence views: they cannot be sliced, resized, or indexed by a
runtime integer.

```zirk
mut index = 1
inmut value = result[index] // error: tuple index is not a compile-time literal
inmut part = result[0:2]    // error: tuples do not slice
```

Tuple destructuring works both at binding sites and inside `match`:

```zirk
inmut (name, score) = ("Ada", 100)

match result {
    ("", 0, false) => show_empty();
    (name, age, active) => show_profile(name, age, active);
}
```

Patterns bind independent projected values. A reference-backed component is
copied according to projection rules.

Equality, hashing, ordering, and cloning are available only when every
component satisfies the corresponding contract. A tuple has structural value
semantics; it does not acquire identity because one component is
reference-backed.

Use a tuple for a small positional product with no domain identity. Use a
record when names matter, an algebraic enum when alternatives matter, and a
class when shared identity and mutation matter.

## API

Fixed-size heterogeneous value with constant `[N]` indexing and structural
value semantics. Tuple literals and `[N]` indexing are delivered
(`array-list-tuple-duration-regex`).

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `Int32` | Compile-time element count | specified |
| `[0]`, `[1]`, … `[N]` | component type | Constant-index projection; the index must be a compile-time integer literal | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `t.to_string()` | `String` | Rendering | specified |

Tuples cannot be sliced, resized, or indexed by a runtime integer. Equality,
hashing, ordering, and cloning exist only when **every** component satisfies
the corresponding contract. Destructuring works at binding sites and inside
`match`.

### Examples

```zirk
inmut result: Tuple(String, Int, Bool) = ("Ada", 37, true);
inmut name = result[0];
inmut (label, score) = ("Ada", 100);

match result {
    ("", 0, false) => show_empty(),
    (n, age, active) => show_profile(n, age, active),
}
```

---

**Previous:** [← Data Types](README.md) · **Next:** [Records →](01-records.md)
