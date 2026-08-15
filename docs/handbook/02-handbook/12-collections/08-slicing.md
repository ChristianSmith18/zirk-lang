# Slicing

A slice describes a contiguous region of a sequence. Its bounds must be valid and its lifetime cannot outlive the underlying safe storage contract.

The syntax is `[start:end:step]`, following Python's ordering. Each component
may be omitted when its default is intended:

```zirk
inmut values = [0, 1, 2, 3, 4, 5];
values[1:4];   // 1, 2, 3
values[::2];   // 0, 2, 4
values[::-1];  // reverse order

inmut text = "Zirk language";
text[0:4];     // "Zirk"
text[::2];     // every second grapheme
```

The end is exclusive. Negative indices count from the end, and a negative step
walks backward. Slicing is available to ordered collections and `String`, with
string indices operating on its public grapheme semantics.

Whether slicing returns a view or an independent collection affects mutation, allocation, and escape behavior and must be documented by each type.

Each concrete type documents whether a slice is a safe view or an independent
value. That representation choice cannot allow a slice to outlive its storage or
create untracked mutable aliasing.

---

**Previous:** [← Indexing](./07-indexing.md) · **Next:** [Safe Access →](./09-safe-access.md)
