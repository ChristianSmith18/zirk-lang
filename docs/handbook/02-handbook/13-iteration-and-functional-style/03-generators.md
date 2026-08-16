# Generators

Generators produce a sequence incrementally instead of constructing every element first. They are useful for large, computed, or potentially unbounded sequences.

```zirk
fn gen count_from(start: Int32): Int32 {
    mut current = start;
    while true {
        yield current;
        current++;
    }
}

for number in count_from(10) {
    stdout.println(number);
    if number == 12 break;
}
```

Calling `count_from` does not run it to completion or build an array. It returns
a generator value. Each request resumes execution after the previous `yield`,
and local state such as `current` remains available between resumptions. The
declared `Int32` is the type of each produced element.

Generator suspension, errors, cancellation, captured state, and resource cleanup require a precise contract. A generator cannot allow a resource to escape a `match with` lifetime indirectly.

Generators implement `Iterator<T>` and `Iterable<T>`. Stopping iteration closes
the generator and performs its required cleanup. Errors, cancellation, captures,
and resources remain governed by the same contracts as ordinary functions.

---

**Previous:** [← Iterator](02-iterator.md) · **Next:** [ map](04-map.md)
