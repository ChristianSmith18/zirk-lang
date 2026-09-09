# Tasks

`task` creates typed managed work and returns `Task<T>`. The bare expression
and block forms are available now.

```zirk
fn load_data(): Int32 { return 42; }

fn main(): Void {
    mut operation: Task<Int32> = task load_data();
    stdout.println(await operation);
}
```

`task expression` evaluates the expression in a child task; `task { ... }`
uses the block's `return` value. Its result must fit in one machine word: a
reference, `String`, `Char`, `Boolean`, `Duration`, `Void`, or a scalar of up
to 64 bits. Captured values are stored in a GC-tracked capture block.

Each handle is consumed once by `await`. A bare discarded task is a diagnostic;
write `_ = task notify();` to make the discard explicit. The executor still
runs that child before the process exits. `task scope`, cancellation and
sibling-failure semantics are specified but remain deferred.

---

**Previous:** [← Concurrency and Parallelism](01-concurrency-vs-parallelism.md) · **Next:** [ await](03-await.md)
