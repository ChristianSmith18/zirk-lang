# ADR-025: Unified Result and Nullable Standard-Library Outcomes

- **Status:** Accepted
- **Date:** 2026-10-22
- **Decision owners:** Zirk language authors
- **Supersedes:** The standard-library API allowance for paired `get` and `get_or_null` methods in ADR-024's surrounding collection guidance

## Context

ADR-024 defines how a `Result<T, E>` projects to `T?` in ordinary value contexts. The collection API nevertheless described both a typed `get` and a nullable `get_or_null`, and HTTP/TLS used `Result<T?, E>` when successful absence had domain meaning. These shapes duplicated outcome handling or made successful absence look similar to operational failure.

## Decision

Standard-library APIs use one of these contracts:

1. When an operation has one recoverable failure outcome, expose one `Result<T, E>` API. Callers may preserve the error with an explicit Result context or `match`, or intentionally project it to nullable/default behavior through ordinary value use and `??`.
2. Keep nullable-only APIs when absence is the complete outcome and there is no parallel Result operation, such as sequence `find`, `first`, `last`, and `pop`.
3. When an operation has multiple successful domain states, encode them with a non-null domain value inside `Result`; retain operational failure in `Error(E)`.

Array and List `get(index)` return `Result<T, IndexOutOfBoundsError>`; Map `get(key)` returns `Result<V, LookupError>`. Their `get_or_null` aliases are removed. Direct bracket access retains its existing controlled-error behavior.

Public HTTP and TLS signatures do not use `Result<T?, E>` for a successful absence state. HTTP uses `HTTPIndirectResult` and `HTTPEventPoll`; TLS uses `TLSNegotiatedALPN` and `TLSPeerCertificateFingerprint`. Private native host-boundary signatures may keep nullable payloads when required by the current ABI; public Zirk wrappers convert those states to domain enums.

This decision narrows the standard-library API guidance around ADR-024. It does not change the language-wide Result projection semantics or remove `??`.

## Consequences

- Lookup callers that need an optional value can write `inmut value: T? = collection.get(key)` or `collection.get(key) ?? fallback`.
- Callers that need lookup diagnostics retain `Result` and match `Ok`/`Error`.
- Existing `get_or_null` collection calls require a source migration to `get`.
- End-of-stream and absent TLS metadata remain distinguishable from transport and TLS failures.
- The public standard-library contract is consistent across collections and transport APIs, while private host ABI work can be migrated independently.
