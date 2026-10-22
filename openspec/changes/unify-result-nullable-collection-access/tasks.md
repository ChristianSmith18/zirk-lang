## 1. Collection lookup contract

- [x] 1.1 Implement `Array.get` and `List.get` as typed Result lookups with negative-index normalization and `IndexOutOfBoundsError`; compiler and IR crates compile, and the compiled CLI accepts typed lookup and nullable projection. Runtime boundary behavior remains unverified.
- [x] 1.2 Implement `Map.get` as `Result<V, LookupError>` and remove native `Map.get_or_null` checking/lowering. The only compiler references left reject the retired spelling with a migration diagnostic.
- [x] 1.3 Migrate existing source, examples, and fixture callers from `get_or_null` to `get`, nullable projection, `??`, or `match`; repository caller search finds no remaining collection alias calls.

## 2. HTTP and TLS multi-state outcomes

- [x] 2.1 Add public non-null HTTP outcome enums and change `indirect_result_url` and `next_event` to return `Result<DomainOutcome, HTTPError>`; the compiled CLI accepts these public signatures.
- [x] 2.2 Add public non-null TLS metadata enums and adapt the public ALPN and certificate fingerprint methods while retaining private host-boundary representations; public method signatures contain no `Result<T?, TLSError>`.
- [x] 2.3 Migrate in-repository Zirk callers to match the new outcome enums; caller search finds no stale nullable handling for the public APIs.

## 3. Normative contract and migration

- [x] 3.1 Add ADR-025 and update collection, HTTP, networking, and standard-library OpenSpecs; strict OpenSpec validation passes.
- [x] 3.2 Update core and standard-library semantic owners, handbook API tables/examples, and feature status; normative API guidance no longer recommends the removed aliases or describes the old public multi-state signatures.
- [x] 3.3 Run focused `cargo check` for sema, IR, CLI, and LLVM codegen; compile public API signatures and projection with `zirk check` and lower the representative API program with `zirk build`. Runtime behavior is tracked separately and remains unverified.
- [ ] 3.4 After the repository documentation is committed, synchronize `../zirk-lang-site`, review its generated diff and run its content validation; if status evidence changes, review its status catalog and use an explicit reviewed audit date.
