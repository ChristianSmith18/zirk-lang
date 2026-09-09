# Projects, modules, and validation

## File and project shape

Source files are `.zrk`; a new application normally has `.zkinit`,
`src/main.zrk`, and `fn main(): Void { ... }`. `.zkinit` is declarative, never
executable code.

```zirk
project {
    name: "my-app";
    version: "0.1.0";
    type: application;
    entry: "src/main.zrk";
}
```

Use quoted local/package imports and unquoted `std.*` imports. `share` exports
a declaration; `import` brings it into scope; `use` exposes an application
global declared in `.zkinit`.

## Verify executable claims

Normative validity and current implementation availability differ. Check
`docs/init/ZIRK_FEATURE_STATUS.md` for the relevant feature, then use the
binary from this checkout rather than relying on `PATH`:

```sh
LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo build -p zirk-cli -p zirk-runtime
./target/debug/zirk check path/to/file.zrk
./target/debug/zirk run path/to/file.zrk
```

For compiler work, use the crate-specific tests prescribed by `AGENTS.md`. For
a documentation example, report the exact check/run command and outcome. If a
feature is normative but unavailable, label it honestly rather than changing
the requested program.

## Tests and standard-library APIs

Use the handbook owner for the concrete API and verify its signature, errors,
permissions and failure behavior. For tests, read the
relevant `docs/handbook/08-testing/` chapter; use `@test`/`@e2e` only when the
feature status confirms them. Do not turn a documentation snippet into an
executable claim without running `check` (and `run` when it has effects).
