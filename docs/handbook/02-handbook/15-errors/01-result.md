# `Result`

`Result<T, E>` represents either `Ok(T)` or `Error(E)` and keeps expected failure in the return type.

It provides Zirk's algebraic equivalent of Go's visible `value, err` outcome:
exactly one variant exists, so “both present” and “neither present” are
impossible. Any `E` is valid; it need not implement the throwable `Error`
contract.

```zirk
fn load(path: String): Result<Document, LoadError> { /* ... */ }
```

Callers can see and compose the failure without hidden control transfer. Choose a specific error type that contains actionable context without exposing secrets.

> **Implementation status:** the current checker recognizes seven structural
> methods — `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`,
> `unwrap`, and `unwrap_error`. `get_or_else`, `map`, `map_error`, `and_then`,
> `or_else`, and `or_throw` are normative but not yet implemented: each needs
> its own method-level type parameter (`map<U>(transform: Fn(T) => U)`), and
> `MethodInfo` has no such field today — only the containing class/contract/enum
> can be generic. `get_or_else` additionally needs `Fn` as a writable parameter
> type, which the language does not have yet.

Wrong-variant unwrap is a programmer assertion and invokes `fatalError`, not a
recoverable exception.

## API

Either `Ok(T)` or `Error(E)` — exactly one variant exists. Any `E` is valid; it
need not implement the throwable `Error` contract.

> **Delivery caveat:** the checker recognizes seven structural methods —
> `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `unwrap`,
> `unwrap_error`. `get_or_else`, `map`, `map_error`, `and_then`, `or_else`, and
> `or_throw` are normative but **not implemented**: each needs a method-level
> type parameter (`map<U>(transform: Fn(T) => U)`), which `MethodInfo` cannot
> express yet, and `get_or_else` additionally needs `Fn` as a writable
> parameter type.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `r.is_ok` | `Boolean` | `true` for `Ok(T)` | implemented |
| `r.is_error` | `Boolean` | `true` for `Error(E)` | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Ok(value)` / `Error(err)` | `Result<T, E>` | Variant constructors | implemented |
| `r.ok_or_null()` | `T?` | Value or `null` | implemented |
| `r.error_or_null()` | `E?` | Error or `null` | implemented |
| `r.get_or(fallback)` | `T` | Value or the supplied default | implemented |
| `r.unwrap()` | `T` | Value; wrong-variant unwrap invokes `fatalError` | implemented |
| `r.unwrap_error()` | `E` | Error; wrong-variant unwrap invokes `fatalError` | implemented |
| `r.get_or_else(fn)` | `T` | Value or `fn(error)` result | specified |
| `r.map<U>(transform: Fn(T) => U)` | `Result<U, E>` | Transform the `Ok` value | specified |
| `r.map_error<F>(transform: Fn(E) => F)` | `Result<T, F>` | Transform the `Error` value | specified |
| `r.and_then<U>(fn: Fn(T) => Result<U, E>)` | `Result<U, E>` | Chaining (flat-map) | specified |
| `r.or_else(fn: Fn(E) => Result<T, E>)` | `Result<T, E>` | Recover from `Error` | specified |
| `r.or_throw()` | `T` | Value, or throw the `E` as a typed exception | specified |

### Examples

```zirk
fn load(path: String): Result<Document, LoadError> { /* ... */ }

match load("doc.zirk") {
    Ok(doc) => render(doc),
    Error(e) => report(e),
}

inmut doc = load("doc.zirk").get_or(Document.empty());
inmut maybe = load("doc.zirk").ok_or_null();   // Document?
```

---

**Previous:** [← Errors](README.md) · **Next:** [ Handling Result](02-handling-result.md)
