# CLI

The implemented CLI includes `init`, `run`, `build`, `check`, `migrate`,
dependency resolution commands, and `permissions inspect|diff|approve|revoke|history`.
Other documented Zirk 1.x commands remain subject to the feature status
catalog.

## Initialize a project

```bash
zirk init
zirk init -y
```

Interactive mode asks one question at a time and shows each default. Enter
accepts the default; optional metadata and target lists can be left blank.
`-y` selects application type, a directory-derived name, version `0.1.0`,
entry `src/main.zrk`, and source root `src`. It works without a terminal.
Neither mode overwrites `.zkinit` or creates source files.

## Run an application

```bash
zirk run
zirk run src/dev.zrk
```

Without a source path, `run` discovers the nearest application `.zkinit` and
executes its configured/default entry. A source path selects the entry for
that invocation while preserving its project dependencies, lockfile, globals,
and requirements. A standalone file uses standalone import rules. Source entry
files execute file-scope statements; no user `fn main` is required.

`check` analyzes without generating code, `build` produces artifacts without
running the application, and `run` builds and starts it. Target selection uses
the explicit CLI `--target`, then an application's `build_targets` list, then
host detection; `run` always chooses the host target.

## Permission commands

`permissions inspect` and `diff` show the dependency requirement tree and exact
saved resource scopes. `history` shows authenticated approval and revocation
events. `revoke` invalidates the saved project record. `approve` does not mint a
blanket grant: first-use consent is requested at the operation with a `y/n`
prompt. A noninteractive protected operation fails closed and can be handled by
Zirk code.

---

**Previous:** [← Diagnostics](07-diagnostics.md) · **Next:** [Formatter](09-formatter.md)
