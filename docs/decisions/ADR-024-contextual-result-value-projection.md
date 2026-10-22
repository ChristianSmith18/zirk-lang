# ADR-024: Contextual `Result` Value Projection

- **Status:** Accepted
- **Date:** 2026-10-21
- **Decision owners:** Zirk language authors

## Context

Zirk keeps expected operational failures in `Result<T,E>` so callers can inspect and compose them. Requiring every routine value use to unpack both variants makes simple reads unnecessarily verbose, while exposing convenience extraction and unwrap methods duplicates the language's exhaustive pattern matching and nullable values.

## Decision

`Result<T,E>` has two usage modes selected by the resolved expression context:

1. A direct `match` of a Result expression, or a value context whose declared type is explicitly `Result<...>`, preserves the complete `Ok(T)` or `Error(E)` value.
2. Every other value-demanding context projects `Ok(value)` to `value` and `Error(_)` to `null`, yielding `T?`. This is a compiler-defined, single-step conversion equivalent to an internal `to_value_or_null()` operation; it is not a public Result method or user-dispatched protocol.

The compiler resolves the surrounding type and generic parameter instantiation before applying projection. Explicitly typed Result variables, parameters, attributes, and returns preserve both variants. Inferred bindings and ordinary value parameters infer from the projected nullable type. `??` applies to the resulting `T?`. Projection discards the error payload and never throws or aborts; code that needs error detail uses direct `match` or preserves the Result with an explicit type annotation.

Projection is supported only when the payload type has a runtime nullable
representation. `Void`, `Decimal`, and opaque `Regex` currently lack one;
value use of `Result` with these payloads is rejected with guidance to use
`match`. Extending nullable runtime representations is a separate language
and runtime change.

A bare Result expression statement remains a compile-time error; `_ = expression` is still the explicit discard. Projection applies to the outer Result only. Printing and interpolation are value-demanding contexts and therefore project before nullable rendering (`<Null>` on failure). An explicitly invoked `result.to_string()` on a Result-typed receiver retains the enum's `Ok(...)` or `Error(...)` representation.

Remove the redundant Result inspection/extraction/fallback/unwrap methods `is_ok`, `is_error`, `ok_or_null`, `error_or_null`, `get_or`, `get_or_else`, `unwrap`, and `unwrap_error`. Remove `Env.get_or_null` and `Env.get_or`; the single environment retrieval API is `Env.get<T>(name): Result<T, EnvironmentError>`. Keep Result composition operations that preserve the Result distinction.

## Consequences

- `Env.get<Int32>("PORT") ?? 8080` uses the fallback for every `Error`, including permission denial, parse failure, missing values, and host errors. A direct `match` remains necessary when these outcomes need different behavior.
- Source code using the removed methods requires migration. This is a language and standard-library compatibility break and must be described in the release notes and public handbook.
- The checker and every lowering target must agree on the single projection boundary and preserve Result values in explicit contexts.
- `Result` expression statements remain must-use, so projection cannot silently turn an unconsumed failure into an ignored nullable value.

## Alternatives Considered

### Require `match` or extraction methods for all value uses

Rejected because routine expected failures remain verbose even when callers only want a nullable value or fallback. It also keeps multiple APIs for the same payload extraction policy.

### Add a public `.to_value_or_null()` method

Rejected because it makes a common value conversion explicit at every call site and can be overridden or confused with ordinary library methods. The conversion is a language-level context rule, not a user-selectable Result combinator.

### Treat `Result` as nullable everywhere

Rejected because callers that require the error value need a stable, lossless representation. Explicit Result contexts and direct `match` preserve that capability.

### Convert errors to exceptions or use a propagation operator

Rejected because it merges distinct failure channels or introduces a separate propagation syntax. This decision does not change `throws`, `RuntimeError`, or the exclusion of `?` propagation from Zirk 1.x.

## Migration

- Replace `result.ok_or_null()` with ordinary Result value use in a nullable context.
- Replace `result.get_or(value)` and `get_or_else(...)` with projection followed by `??` when all failures may use the fallback.
- Replace status checks and unwrap operations with direct `match` when success and error details matter.
- Add `: Result<T,E>` to a binding, parameter, attribute, or return when the Result must survive beyond its immediate expression.
- Replace `Env.get_or_null(name)` and `Env.get_or(name, fallback)` with `Env.get(name)` and either direct matching or nullable projection; document that `??` handles every Error variant alike.
