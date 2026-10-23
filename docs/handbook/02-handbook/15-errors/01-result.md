# `Result`

`Result<T, E>` represents either `Ok(T)` or `Error(E)` and keeps expected failure in the return type. Any `E` is valid; it need not implement the throwable `Error` contract.

There are two ways to consume it. A direct `match`, or a value context explicitly typed `Result<T,E>`, retains both variants. In every other value-demanding context the compiler projects `Ok(value)` to `value` and `Error(_)` to `null`, producing `T?`. This projection intentionally discards the error payload.

```zirk
fn load(path: String): Result<Document, LoadError> { /* ... */ }

// Concise value use: inferred type is Document?.
inmut maybe_document = load("doc.zirk");

// A nullable fallback handles every Error variant.
inmut document = load("doc.zirk") ?? Document.empty();

// Preserve the typed error for inspection.
inmut result: Result<Document, LoadError> = load("doc.zirk");
```

Use `match` when the caller needs to distinguish missing data, permission denial, parsing failure, or another error:

```zirk
match load("doc.zirk") {
    Ok(document) => render(document),
    Error(error) => report(error),
}
```

Projection is equivalent to an internal `to_value_or_null()` operation, not a public method. It maps every `Error(error)` to `null`, without examining `error`, throwing, or calling `fatalError`. `??` therefore uses its fallback for every error, not only absence. A direct match or explicit Result annotation is the lossless path.

Projection is available only when `T?` has a runtime representation. `Void`,
`Decimal`, and opaque `Regex` do not currently have one, so use `match` for
those payloads; ordinary value use is a compile-time error.

A Result used as a standalone expression statement is an effect: `Ok(value)` discards the value, while `Error(error)` throws. A `Throwable` error is thrown unchanged; any other error becomes a catchable `NativeError` with the error's text as its reason. Use `match` or keep the Result in an explicitly typed binding when you want to inspect an error without throwing. Write `_ = operation()` only when deliberately discarding both outcomes.

## API

| Surface | Contract | Status |
| --- | --- | --- |
| `Ok(value)` / `Error(error)` | The two `Result<T,E>` variants | Implemented |
| Direct `match result` | Exhaustive inspection of the original variants | Implemented |
| Contextual value use | `Ok(T)` becomes `T`; `Error(E)` becomes `null`; result type is `T?` | Implemented |
| `map`, `map_error`, `and_then`, `or_else`, `or_throw` | Composable transformations that preserve `Result` | Specified; implementation pending |

The removed methods `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, and `unwrap_error` duplicated direct matching or contextual projection. Use `match` for inspection and ordinary value use with `??` for a fallback.

Printing and interpolation are value contexts: a successful Result prints its value and a failed Result prints the nullable marker `<Null>`. Calling `result.to_string()` directly on a Result-typed value keeps the enum representation, such as `Ok(5)`.

---

**Previous:** [← Errors](README.md) · **Next:** [Handling Result](02-handling-result.md)
