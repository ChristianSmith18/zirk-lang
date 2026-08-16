# `std.io`

`std.io` exports `stdin`, `stdout`, and `stderr`. Importing one of these standard
objects makes its unambiguous convenience operations available directly.

```zirk
import { stdin, stdout, stderr } from std.io;
println("User: {user.name}");
stdout.println("User: {user.name}");
```

Both calls target `stdout.println`. The qualified spelling is required when a
local declaration or another import also uses the name `println`. Likewise,
error-output operations can be qualified through `stderr` whenever the direct
name would be ambiguous.

Input supports lines, characters, and bytes. EOF is data—not an exception—and uses `ReadResult<T>` or an equivalent `Data`, `Eof`, `Error` algebraic result. Async reads are task-suspending and cancelable.

---

**Previous:** [← Common API Contracts](01-common-api-contracts.md) · **Next:** [ std.fs](03-std-fs.md)
