# `import`

`import` names published declarations a file uses.

```zirk
import { stdin, stdout, stderr } from std.io;
import { User } from "./domain/user";
```

Logical packages are unquoted; local paths are quoted. Imports are lexical
dependencies, not runtime code execution. Missing, private, or ambiguous names
produce resolution diagnostics.

An imported module opens its published members when the name is unambiguous:

```zirk
import { stdout } from std.io;

println("Hello"); // Resolves to `stdout.println`.
```

If the file already declares or imports another `println`, qualify the standard
member to make the choice explicit:

```zirk
stdout.println("Hello");
```

The same rule applies to local, external and standard-package modules. A
collision has no source-order tiebreak; qualify it instead:

```zirk
import { terminal } from acme.cli;

terminal.clear();
```

`import { println } from std.io::stdout;` imports only that member. Module
aliases change the qualifier (`import { terminal -> console } from acme.cli;`
then `console.clear()`); `use` does not open modules.

---

**Previous:** [← share](02-share.md) · **Next:** [ Import Aliases](04-import-aliases.md)
