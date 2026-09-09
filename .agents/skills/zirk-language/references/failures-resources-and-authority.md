# Failures, resources, and authority

Choose failure handling before writing an API:

| Situation | Use |
| --- | --- |
| Expected operational outcome | `Result<T, E>` with exhaustive `match` or a Result method |
| Recoverable exceptional path | `throws` plus `try`/`catch`, or declare propagation |
| Unrecoverable invariant | `fatalError` |
| Owned external resource | `match with` |

There is no `?` propagation. An ignored `Result` is a compile error. `finally`
always runs but cannot replace an active outcome with control transfer.

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
