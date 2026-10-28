# `match with`

`match with` handles acquisition and scopes the acquired resource.

```zirk
import { File, OpenOptions } from std.fs::fs;
import { Path } from std.path::path;

match File.open(Path.of("data.txt"), OpenOptions.read_only()) with file {
    Ok(file) => file.read_line();
    Error(error) => report(error);
}
```

The file closes before the expression delivers its value. Closure occurs on
normal completion, `return`, a propagating exception, and a `break` or
`continue` that leaves the scope.

Grouped acquisition (`match a with x, b with y { ... }`) opens resources
left-to-right and closes them right-to-left, including earlier resources when
a later acquisition fails.

> **Implementation status:** single and grouped acquisition, deterministic
> cleanup, `ResourceFailure` body/close error composition, explicit transfer,
> use-after-transfer rejection, and cancellation-aware cleanup are delivered
> end-to-end. Cancellation remains cooperative, so a non-cooperative operation
> can delay cleanup. See the [feature status catalog](../../../init/ZIRK_FEATURE_STATUS.md).

---

**Previous:** [← The Resource Contract](01-resource-contract.md) · **Next:** [ Resource Transfer](03-resource-transfer.md)
