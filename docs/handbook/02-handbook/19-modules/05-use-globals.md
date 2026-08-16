# `use` and Globals

Only applications may declare globals, inside `globals` in `init.zrk`. Each source consumer opts in with `use`.

```zirk
use APP_NAME;
```

`use` does not import code. Mutable globals accessed from `parallel` or `thread` require `sync` or `Atomic<T>`. Libraries cannot define application globals.

---

**Previous:** [← Import Aliases](04-import-aliases.md) · **Next:** [ Public API](06-public-api.md)
