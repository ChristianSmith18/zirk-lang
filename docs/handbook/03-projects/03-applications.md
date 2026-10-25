# Applications

An application owns a project entry, may declare typed globals, and may list
capability requirements. `requires` records what code may need; it does not
grant user consent. The user approves exact resources when protected code
reaches them.

```text
project {
    name: "app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}

requires {
    environment.read;
}
```

If `entry` is omitted, it defaults to `src/main.zrk`. The entry file executes
its top-level statements; it does not need `fn main`. Use `zirk run` to select
this configured entry, or pass a source path to override it for one run while
keeping the project context.

---

**Previous:** [← .zkinit](02-zkinit.md) · **Next:** [Libraries](04-libraries.md)
