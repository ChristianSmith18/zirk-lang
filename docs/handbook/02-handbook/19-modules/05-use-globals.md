# `use` and Globals

Only applications may declare globals, inside `globals` in `.zkinit`. Each source consumer opts in with `use`.

```zirk
use APP_NAME;
```

`use` does not import code. The compiler rejects mutable reference captures at
the implemented `parallel` boundary, but this is not a general check for
unsynchronized mutable-global access. Managed `thread` and the public mutex,
rwlock, semaphore, barrier, and once APIs remain unavailable; scalar
`Atomic<T>` is available for supported target widths. Libraries cannot define
application globals.

---

**Previous:** [← Import Aliases](04-import-aliases.md) · **Next:** [ Public API](06-public-api.md)
