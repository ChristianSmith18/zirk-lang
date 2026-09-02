# ADR-011 — Identity, equality and normalization of `String`

- **Status:** accepted
- **Date:** August 15, 2026
- **Phase:** 2 (closing)

## Context

`ZIRK_LANGUAGE_SPEC.md` section 4 says that `==` compares structurally and that `is` compares the same instance. For `String` that isn't enough to settle an implementation, because Unicode allows writing the same perceived text with different bytes:

```text
"Å"   →  U+00C5                (NFC, composed form)
"Å"   →  U+0041 U+030A         (NFD, decomposed form)
```

That's four bytes versus five. A user who types both in their editor sees exactly the same thing, and most likely doesn't know which one produced each: it depends on the operating system, the input method, and what the text passed through before reaching the file.

The current implementation compares bytes, so today it would answer `false`. And defining `String` as a sequence **of graphemes** makes that answer inconsistent with the rest of the type: if `length` counts graphemes and indexing returns graphemes, equality can't reason in bytes.

## Decision

**`is` compares referents. `==` compares content, and is indifferent to the normalization form.**

```text
mut a = "Å";               // NFC
mut b = "Å";               // NFD, same perceived text
mut c = a;

a == b   // true   — same content
a is b   // false  — different referents
a is c   // true   — same referent
```

The hash is derived from the canonical form. If it were derived from bytes, two keys equal under `==` would fall into different buckets and `Map<String, _>` would contradict the operator — a map where `m[a]` and `m[b]` are separate entries while `a == b` is true.

### How it's paid for

The rule is expensive if implemented naively — normalizing both sides on every comparison — and cheap if the paths are ordered by frequency:

1. **Same handle** → equal. This is also `is`'s answer. A pointer.
2. **Identical length and bytes** → equal. A `memcmp`, with no memory allocation. This is the overwhelmingly most common case.
3. **Both marked canonical, different bytes** → different. Two distinct canonical forms are different texts, by definition.
4. **Everything else** → incremental canonical comparison, without materializing normalized copies when avoidable.

The handle stores, alongside the bytes, what's needed to reach the first three steps early: `is_ascii`, `normalization`, `grapheme_count` and `hash`. An ASCII string admits no distinct equivalent forms, so `is_ascii` guarantees step 2 decides.

**Literals are normalized at compile time** and reach the runtime marked as canonical. It's work done once, on a machine that isn't in a hurry, and it turns comparison between literals — the most common case there is — into a byte comparison.

## Alternatives considered

**Just compare bytes.** That's what exists today and it's the fastest possible option. It's discarded because it makes `"Å" == "Å"` depend on which keyboard each literal was typed with, which is exactly the kind of surprise the language sets out not to have. A user can't debug that: both literals look identical on their screen.

**Always normalize on construction.** Every `String` enters canonical form and equality goes back to being a `memcmp`. Tempting, but it pays normalization on every string built at runtime — including ones nobody ever compares — and also destroys information: a program that reads a file and writes it back out would alter bytes it wasn't asked to alter. It's discarded more for that second reason than for the cost.

**Expose normalization to the user** (`a.normalized() == b.normalized()`). Shifts the problem to whoever writes the program and guarantees it gets forgotten, because the failing case is precisely the one that isn't visible. It also forces `Map<String, _>` to document which of the two equalities it uses.

## Consequences

- The runtime gains a dependency on Unicode canonical equivalence data. It's limited to what canonical equivalence needs, which is considerably less than full Unicode, and is only touched in step 4.
- The compiler gains a minor responsibility: normalizing literal text when emitting it. It doesn't touch the boundary of [ADR-005](./ADR-005-representacion-string.md), because it operates on the literal's text and not on the `String`'s representation.
- The handle's cached fields — `normalization`, `grapheme_count`, `hash` — become invalidatable state. As long as `String` isn't mutable there's no path that desyncs them; whichever phase introduces mutation takes on invalidating them, and that's part of its contract and its tests.
- The same discipline will apply to `Char`: a grapheme compared by canonical content, for the same reasons and with the same fast paths.
