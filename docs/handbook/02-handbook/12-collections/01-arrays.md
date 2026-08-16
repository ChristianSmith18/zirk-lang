# Arrays

An array stores a fixed number of ordered values sharing one element type. Its
length never changes after construction.

```zirk
inmut names: Array<String> = ["Ada", "Grace", "Linus"];
```

The initializer infers the fixed length. Zirk accepts both generic and bracket
type forms, but `String[n]` is the official convention when the size is written:

```zirk
mut inferred: String[] = ["Ada", "Grace", "Linus"]; // length 3
mut names: String[8];                                // canonical explicit form
mut also_valid = Array<String>(8);                   // supported alternative
```

`Array<String>(n)` and `String[n]` allocate an array whose fixed length is `n`.
Elements may be replaced when the array value is mutable, but elements cannot
be appended or removed. Use `List<T>` when length must grow or shrink.

Indexing uses integer positions and performs bounds checks in safe code. Iteration order follows array order. Mutation depends on the binding and collection contract; an `inmut` binding does not automatically imply deep immutability.

All Zirk arrays are fixed-size. A declaration that makes the size part of its
type communicates an additional compile-time contract; it does not select a
different resizable collection.

Array equality is element-wise when `T` supports equality. Assignment aliases the same array; it does not copy elements. Slices and iterators preserve bounds and alias permissions, and mutation through any view is rejected when the source is `inmut::strict`.

---

**Previous:** [← Collections](README.md) · **Next:** [ Fixed Arrays](02-fixed-arrays.md)
