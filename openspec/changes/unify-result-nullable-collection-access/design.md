## Context

See `proposal.md` for motivation. The language already projects native `Result<T, E>` to `T?` in ordinary value contexts. The collection contract still lists paired `get`/`get_or_null` operations, while only `Map.get_or_null` currently has compiler and IR support. Public HTTP/TLS source also contains four `Result<T?, E>` signatures whose `Ok(null)` states carry domain meaning. The companion website imports the handbook and audited status from this repository after a clean commit.

## Goals / Non-Goals

**Goals:**

- Make `get` the single recoverable lookup API for Array, List, and Map, with typed errors preserved in explicit Result contexts.
- Remove the equivalent nullable lookup aliases and give source callers a migration path through contextual projection and `??`.
- Preserve successful absence states in HTTP/TLS without making them nullable payloads of public Result APIs.
- State a repeatable standard-library rule for nullable-only operations and multi-state Result APIs.

**Non-Goals:**

- Change direct indexing (`value[index]`) or its controlled exception behavior.
- Remove nullable-only `find`, `first`, `last`, `pop`, metadata accessors, or any operation without a Result-returning counterpart.
- Change the language-wide behavior that projects every `Error(E)` to `null` in an ordinary value context.
- Add a generic `Option<T>` type or change private host ABI layouts solely to implement the public domain outcomes.
- Change mutating lookup helpers such as `Map.get_or_insert_with`.

## Decisions

### Use `get` as the only safe collection lookup

Array and List `get(index)` return `Result<T, IndexOutOfBoundsError>`; Map `get(key)` returns `Result<V, LookupError>`. Ordered collection indexes are normalized using the existing negative-index rule before bounds are checked. Missing or invalid indexes produce `Error(IndexOutOfBoundsError)`; absent map keys produce `Error(LookupError)`. Explicit `Result` annotations and `match` preserve these errors, while nullable contexts use the existing general projection.

`get_or_null` is removed from the public compiler surface and contract. The compiler reports a focused migration diagnostic pointing to `get`, nullable projection, `??`, or `match`. Direct bracket indexing remains the strict convenience operation and continues to raise its existing controlled bounds error.

This is preferable to retaining both names because these collection lookups have one recoverable failure state: absence or invalid bounds. A second method would encode the same lossy projection. A future operation with additional failure causes must retain those causes in `Result`; callers who intentionally erase them can do so through projection.

### Keep nullable-only APIs where no Result operation exists

`find`, `first`, `last`, and `pop` remain nullable because their current contract represents absence as the sole non-success state and has no Result sibling. `pop` also mutates the sequence. `Map.get_or_insert_with` remains a mutating insertion operation. Header/form/route getters and optional metadata accessors likewise remain nullable when they have no same-operation Result alternative.

### Represent public multi-state outcomes with non-null domain values

Public `HTTPEventStream.next_event` returns `Result<HTTPEventPoll, HTTPError>`, where `HTTPEventPoll` has `Event(HTTPEvent)` and `End`. `HTTPResponse.indirect_result_url` returns `Result<HTTPIndirectResult, HTTPError>`, where `HTTPIndirectResult` has `Direct` and `Redirect(URL)`. TLS metadata uses `TLSNegotiatedALPN.NotNegotiated`/`TLSNegotiatedALPN.Negotiated(String)` and `TLSPeerCertificateFingerprint.Unavailable`/`TLSPeerCertificateFingerprint.Sha256(String)` inside `Result`.

The private compiler/runtime host boundary may continue to encode optional metadata as nullable payloads if that is its existing ABI. The public `.zrk` wrapper maps the host result to the non-null domain enum without turning host errors into successful absence. This avoids a backend ABI change while keeping the public contract unambiguous.

### Record the standard-library criterion in normative owners

Update the collection, HTTP, networking, and standard-library OpenSpecs; `CORE_LANGUAGE_SEMANTICS.md`; `ZIRK_STDLIB_SPEC.md`; ADR-024's follow-up decision record; collection/HTTP/Net handbooks; status and migration guidance. The standard-library requirement distinguishes (a) nullable-only absence with no Result twin, (b) one Result API with contextual projection, and (c) a Result whose non-null domain payload distinguishes multiple successful states.

## Risks / Trade-offs

- [Risk] Collection lookup is a public API break and existing callers may depend on `get_or_null`. → Mitigation: provide a direct mechanical migration pattern and compiler diagnostics; update all in-repository examples and fixtures.
- [Risk] Collection `get` adds generic Result construction to three native collection families. → Mitigation: build the `Ok` and typed `Error` variants in checked IR using the registered native error types; keep strict indexing lowering unchanged.
- [Risk] Callers may confuse `Ok(End)` with `Error` after the SSE signature change. → Mitigation: use named domain enum variants and update examples to match both Result and poll-state layers explicitly.
- [Risk] Private TLS host methods still use nullable payloads and could be mistaken for public API. → Mitigation: keep them private, document their role as host ABI adapters, and expose only the non-null enums from `TLSStream`.
- [Risk] Existing handbook statements and ADR-024 explicitly permit collection `get_or_null`. → Mitigation: record this as a superseding decision in a new ADR and update the normative semantic/spec owners in the same change.

## Migration Plan

1. Add the new public collection `get` implementations and remove `get_or_null` from compiler dispatch and IR.
2. Convert repository callers from `get_or_null` to `get` consumed as `T?`, with `??`, or through `match` as appropriate.
3. Add non-null domain outcome enums to public HTTP/TLS APIs and adapt their private host or parser outcomes.
4. Update specifications, ADR, handbook, feature status, examples, and current limitations from the resulting implementation state.
5. After the source repository changes are committed, synchronize the companion website from the clean commit and review its generated content.

Rollback, if needed before a stable release, restores the prior collection aliases and public enum signatures together with their source/docs; no persisted data or wire format changes are involved.
