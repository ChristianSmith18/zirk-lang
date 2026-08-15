# Pipelines

Pipelines arrange transformations in reading order.

Without a pipeline, pure functions nest from the inside out:

```zirk
inmut result = reduce(map(filter(users)));
```

The pipe passes the result on its left into the next pure function, making the
same data flow readable from top to bottom:

```zirk
inmut result = users
    |> filter(is_active)
    |> map(to_summary)
    |> reduce(merge_summaries);
```

Each stage remains a normal typed call: generic constraints, errors, effects, and lazy/eager behavior still apply. A pipeline is not an optimization guarantee.

This differs from method chaining:

```zirk
inmut result = users.filter(is_active).map(to_summary).reduce(merge_summaries);
```

Here `filter`, `map`, and `reduce` are methods supplied by the collection or
iterable contract. In a pipeline they can be independent reusable functions.
Both styles may look similar, but pipelines do not require the previous value's
class to own every operation.

Use pipelines when data movement is the story. Use explicit statements when branching, recovery, resource scopes, or intermediate diagnostics deserve names.


---

**Previous:** [← Lazy Operations](./07-lazy-operations.md) · **Next:** Classes and Objects *(next handbook unit)*
