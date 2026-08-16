# `import`

`import` names published declarations a file uses.

```zirk
import { stdin, stdout, stderr } from std.io;
import { User } from "./domain/user";
```

Standard modules are unquoted; local paths are quoted. Imports are lexical dependencies, not runtime code execution. Missing, private, or ambiguous names produce resolution diagnostics.

Members imported from the standard library also expose their ordinary
convenience operations directly when the name is unambiguous:

```zirk
import { stdout } from std.io;

println("Hello"); // Resolves to `stdout.println`.
```

If the file already declares or imports another `println`, qualify the standard
member to make the choice explicit:

```zirk
stdout.println("Hello");
```

This convenience applies to standard-library imports, whose APIs are known to
the compiler. Local and package objects do not automatically inject all their
methods into file scope.

---

**Previous:** [← share](02-share.md) · **Next:** [ Import Aliases](04-import-aliases.md)
