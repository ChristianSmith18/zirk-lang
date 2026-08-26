## 1. Checker (zirk-sema)

- [ ] 1.1 Remove the `not_lowered` gate in `check_conformance` for a `record`/`value class` with non-empty `implements` (`crates/zirk-sema/src/checker.rs`, ~line 1650-1658).
- [ ] 1.2 Type the implicit conversion from a `record`/`value class` value to a reference of a contract type it implements (`return`, assignment, argument-passing, collection-element positions — the same set of positions a class instance's own contract-typed conversion already covers; confirm the actual current list rather than assuming it).
- [ ] 1.3 Checker unit tests: a `value class`/`record` implementing an interface types correctly when assigned/returned/passed as that interface; a value type NOT implementing a named interface is still rejected the same way a class would be; calling an interface method through the interface-typed reference type-checks.

## 2. IR (zirk-ir)

- [ ] 2.1 Define the box construction shape (design D1): a new IR allocation representing the boxed value — header plus the value type's own existing field layout copied in, with a compile-time-built descriptor for the `(value type, implemented contracts)` pair (method table entries in the same index order a class's own descriptor uses, `contract_instances` registered the same way `check_conformance` already registers them for a class).
- [ ] 2.2 Confirm (design D3) the box is read-only: no store/write instruction is ever generated against a boxed value's fields — only the copy-in at construction and reads through the existing contract-dispatch path.
- [ ] 2.3 IR-level tests: the conversion lowers to a box-construction instruction; the boxed value's fields match the source value's fields at construction; no write instruction targets the box anywhere in the lowered IR for a test program that never attempts one (a negative check, not just absence of a compile error).

## 3. Codegen (zirk-codegen-llvm)

- [ ] 3.1 Build a per-`(value type, contracts)` static descriptor (method table + `contract_instances`) reusing the existing class-descriptor-building code path (`crates/zirk-codegen-llvm/src/emit.rs`, ~line 126-150's table-building logic), parameterized over a value type's own method/contract data instead of a class's — do not write a second, divergent descriptor builder (design's own explicit mitigation).
- [ ] 3.2 Emit the box construction: allocate through the existing `zirk_rt_alloc` path (ordinary collector-tracked allocation, same header shape every object already has), copy the value's fields in, install the descriptor.
- [ ] 3.3 Confirm `CallContract`'s existing dispatch lowering needs zero changes to correctly call through a boxed value's descriptor (design D2) — verified by a real compiled-and-run test, not by inspection alone.

## 4. Cross-cutting correctness tests (do not skip)

- [ ] 4.1 End-to-end `.zrk` fixture: a `value class` implementing an interface, held through an interface-typed variable, with a call through that variable dispatching to the value class's own method and returning the correct result.
- [ ] 4.2 End-to-end `.zrk` fixture: a `record` implementing an interface, same shape as 4.1.
- [ ] 4.3 End-to-end `.zrk` fixture: a heterogeneous collection (e.g. a `List` or an array, whichever is available) holding two different concrete value-class adopters of the same interface, iterated and dispatched correctly per element — the scenario that specifically rules out static monomorphization as a substitute for boxing (design D1's own "Alternative considered").
- [ ] 4.4 Confirm (design D3/Risk) `is` between two separately-boxed equal values does not report `true` by any accident of implementation, and that no write path exists through the interface-typed reference — an explicit test attempting (and failing to find) a mutation path, not just an absence of a positive test.
- [ ] 4.5 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 5. Documentation and status sync

- [ ] 5.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md` — remove "value-type contract dispatch" from remaining Phase 3 debt once delivered.
- [ ] 5.2 Check `docs/handbook` for any existing value-class/interface teaching material with a stale "not yet implemented" caveat — reconcile.
- [ ] 5.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 5.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 6. OpenSpec close-out

- [ ] 6.1 Run `openspec validate fase-3-value-type-contract-dispatch` before archiving.
- [ ] 6.2 Archive the change once implementation, tests, and documentation sync are complete.
