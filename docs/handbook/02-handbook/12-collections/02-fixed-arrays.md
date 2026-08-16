# Fixed Arrays

Every Zirk array has a fixed length. This chapter focuses on declarations that
make that length part of the visible type or declaration contract. They suit
protocol fields, native layout, SIMD-adjacent data, and algorithms requiring
exact dimensions.

Unlike a dynamic collection, it cannot grow. A size mismatch is a compile-time error when statically known.

```zirk
mut bytes: UInt8[16];
mut matrix: Float64[4];
```

`T[n]` is the official explicit-size convention. `Array<T>(n)` remains a
supported construction alternative, and an array literal infers its fixed
length.

Fixed size does not waive bounds checking in safe code.

---

**Previous:** [← Arrays](01-arrays.md) · **Next:** [ Lists](03-lists.md)
