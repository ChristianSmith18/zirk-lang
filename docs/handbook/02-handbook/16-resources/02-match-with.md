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

Grouped acquisition (`match a with x, b with y { ... }`, opening left-to-right
and closing right-to-left with earlier resources closed before a later open's
error branch runs) is normative but not implemented — one `match ... with`
manages a single acquisition today. Nesting one `match with` inside another
arm's body works, since each is an ordinary expression.

> **Implementation status:** cancellation-triggered closure is normative but
> not implemented — Zirk has no concurrency primitives yet (roadmap Phase 5),
> so there is nothing to cancel a resource scope.

---

**Previous:** [← The Resource Contract](01-resource-contract.md) · **Next:** [ Resource Transfer](03-resource-transfer.md)
