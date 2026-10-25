# Project entry

An application starts from a source file, not a required function. The
manifest's `project.entry` selects the default file (`src/main.zrk` when
omitted), and its top-level executable statements run once in source order.
Function and type declarations define code; they are not called just because
they appear in the entry. A function named `main` is ordinary Zirk code.

```zirk
import { stdout } from std.io;

inmut message = "ready";
stdout.println(message);
```

`zirk run` discovers the nearest application `.zkinit` and uses its entry. Use
`zirk run path/to/file.zrk` to select a different file for one invocation; when
the file belongs to a project, dependencies, lockfile, globals, and capability
requirements still come from that project. An explicit file outside a project
runs under standalone import rules.

Imports are static and must appear before executable file-scope statements.
The compiler validates the full import graph before running code. Imported
modules initialize once, depth-first in import source order, before their
importer. The selected application entry runs after its imported modules.
Initialization cycles that require executing a partially initialized module
are rejected before startup.

The native executable still has an internal platform entry function to
initialize and shut down the runtime. Zirk source does not need a `fn main`
wrapper.

---

**Previous:** [← Libraries](04-libraries.md) · **Next:** [Globals](06-globals.md)
