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

Omitted positive-step bounds mean start `0` and end `length`; omitted
negative-step bounds mean start at the last element and stop just before the
first. Step zero and explicitly out-of-bounds values are errors. Zirk does not
clamp explicit bounds as Python does.

Every slice is a deep independent collection. Slice assignment requires an
equal-sized replacement. Use an explicit view API to share storage, or a named
structural operation such as `splice` when a list must change length.

---

**Previous:** [← Indexing](07-indexing.md) · **Next:** [ Safe Collection Access](09-safe-access.md)
