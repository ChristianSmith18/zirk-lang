# Failures, resources, and authority

Choose failure handling before writing an API:

| Situation | Use |
| --- | --- |
| Expected operational outcome | `Result<T, E>`; use `match` to retain or distinguish errors |
| Recoverable exceptional path | `throws` plus `try`/`catch`, or declare propagation |
| Unrecoverable invariant | `fatalError` |
| Owned external resource | `match with` |

There is no `?` propagation. An ignored `Result` is a compile error. `finally`
always runs but cannot replace an active outcome with control transfer.

An explicit `match` or a binding annotated `Result<T, E>` preserves both
variants. In an ordinary value context, `Result<T, E>` projects to `T?`; `??`
uses its fallback for every `Error`, including permission, parsing, and host
errors. Use that concise form only when all failures may be treated alike:

```zirk
inmut port = Env.get<Int32>("PORT") ?? 8080;
```

If the caller must distinguish absence, invalid input, permission denial, or a
host failure, match the original result:

```zirk
match Env.get<Int32>("PORT") {
    Ok(port) => start_server(port);
    Error(error) => report(error);
}
```

Do not recommend removed Result inspection, extraction, fallback, or unwrap
helpers (`is_ok`, `ok_or_null`, `get_or`, `unwrap`, and related methods) or
collection `get_or_null`. Keep nullable-only operations when absence is their
complete outcome and no Result-returning twin exists. Do not confuse such APIs
with testing assertions like `assert.is_ok`, which inspect an assertion
subject rather than consume a Result as a library convenience method. When one
operation has multiple successful states, use a non-null domain value inside
`Result` to keep those states distinct from `Error`.

```zirk
mut first_line: String = match File.open("data.txt") with file {
    Ok(file) => file.read_line();
    Error(error) => "";
};
```

`match with` closes the resource on every normal or exceptional exit. Do not
use general `defer` or allow a managed resource to escape without explicit
transfer.

Applications grant permissions in declarative `.zkinit`; libraries state
`requires`. Declaring a permission is not consent. Read the governing semantic
source before adding filesystem, network, process, or build authority.
