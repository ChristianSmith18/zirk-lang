# Tuples

A tuple is a fixed-size heterogeneous value. Its type uses `Tuple(...)`; its
literal uses parentheses.

```zirk
inmut result: Tuple(String, Int, Bool) = ("Ada", 37, true)
inmut name = result[0]
inmut active = result[-1]
```

The index must be a compile-time integer constant so the checker knows the
result type. Negative constants count from the end. A tuple is not a sequence
view: it cannot be sliced, resized, or indexed by a runtime integer.

```zirk
mut index = 1
inmut value = result[index] // error: tuple index is not compile-time constant
inmut part = result[0:2]    // error: tuples do not slice
```

Tuple destructuring creates independent projected values:

```zirk
inmut (name, score) = ("Ada", 100)
```

Equality, hashing, ordering, and cloning are available only when every
component satisfies the corresponding contract. A tuple has structural value
semantics; it does not acquire identity because one component is
reference-backed—the component is copied according to projection rules.

Use a tuple for a small positional product with no domain identity. Use a
record when names matter, an algebraic enum when alternatives matter, and a
class when shared identity and mutation matter.

---

**Previous:** [← Data Types](README.md) · **Next:** [Records →](01-records.md)
