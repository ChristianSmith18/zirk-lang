# `parallel`

`parallel { ... }` opens a CPU-bound region on a separate worker pool, not the
cooperative executor `concurrent`/`spawn` use.

```zirk
parallel {
    for item in files {
        compress(item);
    }
}
```

An optional `;`-separated header sets `cores` before the block:

```zirk
parallel; cores: 4 { for item in files { compress(item); } }
parallel; cores: -1 { for item in files { compress(item); } }   // leave one core free
parallel; cores: 1..=8 { for item in files { compress(item); } } // runtime picks
```

`cores: N` (`N > 0`) uses exactly `N` worker threads. `cores: -N` uses the
detected core count minus `N` — a runtime error if that leaves zero or fewer.
`cores: A..=B` lets the runtime choose within the range. Absent, the region
uses every detected core. `cores: 0` is a compile-time error.

A `parallel` region performs no I/O and no suspension: `Timer.*`, `spawn`,
`concurrent { }`, and printing are all rejected inside one — pool threads have
no safe points for them. A value or projection copies into the region; a
`inmut::strict` reference may be shared; a `mut` reference the enclosing scope
still holds may not be captured (see
[Transfer, Sharing, and Captures](17-transfer-sharing-and-captures.md)).

`parallel { }` may also be used where an expression is expected, and then
types as its final expression:

```zirk
inmut total = parallel { compute() };
```

---

**Previous:** [← Channels](07-channels.md) · **Next:** [ parallel for](09-parallel-for.md)
