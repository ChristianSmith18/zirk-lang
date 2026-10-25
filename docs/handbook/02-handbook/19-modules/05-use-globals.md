# Application globals

Only applications declare typed globals in the `.zkinit` `globals` block.
Source files do not opt into names with `use`; they read and write through the
compiler-checked `Global` API:

```zirk
inmut settings = Global.get("settings");
Global.set("retry_limit", 4);
mut changes = Global.watch("retry_limit");
```

The compiler infers each type from the literal key and the project manifest.
Packages cannot declare application globals. See the
[project globals chapter](../../03-projects/06-globals.md).

---

**Previous:** [← Module Initialization](04-module-initialization.md) · **Next:** [Package Imports](06-package-imports.md)
