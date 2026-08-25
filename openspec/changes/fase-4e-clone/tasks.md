## 1. Spec wording correction

- [ ] 1.1 Confirm `Clone` is authoritative (see proposal's "Why") — no further research needed, just apply: the delta specs in this change already correct `Cloneable` → `Clone` at `zirk-grammar` (Herencia y contratos combinados) and `zirk-type-system` (Clon mutable desde referencia strict). Verify no other `Cloneable` occurrence exists anywhere under `openspec/specs/` before implementation (`grep -rn Cloneable openspec/specs/`).

## 2. Checker (zirk-sema)

- [ ] 2.1 Add the `Clone` trait as a compiler-recognized built-in contract (parallel treatment to how `Weak<T>`'s element-type restriction reused `is_reference_type` — check the actual current representation of built-in contracts/traits before assuming a shape).
- [ ] 2.2 Derivation eligibility (design D1): for a `class`/`record`/`value class`, walk its field types transitively; derive `Clone` iff every field type is itself `Clone` (recursing through user classes, rejecting on `Pointer<T>`, `Resource`-derived types, `Task<T>`, synchronization types, or any field whose own type fails derivation). No partial/best-effort derivation.
- [ ] 2.3 Compile-time rejection (design D4): at a `.clone()` call site or a `T from Clone` generic-bound check, if the receiver's type is not `Clone`, reject with a new diagnostic naming the specific field/transitive path that broke the chain. New code in the `E04xx` range, continuing after `E0449` (`WEAK_DISALLOWED_REFERENT`) — confirm the actual next free code before assigning.
- [ ] 2.4 Type `.clone(): T` on any `Clone`-eligible receiver type `T`.
- [ ] 2.5 Checker unit tests: a class with only value-typed fields derives `Clone`; a class with a `Clone` class-typed field derives `Clone`; a class with a `Pointer<T>`/`Resource`/`Task<T>` field does not derive `Clone` and using `.clone()` on it is rejected naming that field; a class with a non-Clone nested class field is rejected naming the transitive path; `T from Clone` generic bound accepted/rejected correctly.

## 3. IR (zirk-ir)

- [ ] 3.1 Define graph-clone IR instructions (design D2): `CloneBegin` (only at the outermost `.clone()` call site), `CloneLookup`/`CloneRecord` (memoization table operations), `CloneAlloc` (allocate the new object, record its mapping before recursing into fields), `CloneEnd` (release the memoization table at the outermost call's return). Reuse the existing per-class field-layout table (`gc_reference_paths` or equivalent — check its actual current name) that mark's tracer already uses, as clone's own field-walk driver — do not build a second copy of "how to enumerate a class's reference-typed fields."
- [ ] 3.2 Lower `x.clone()`: outermost call wraps with `CloneBegin`/`CloneEnd`; for the root and every reference-typed field recursively, `CloneLookup` first (handles the root-revisited-via-cycle case and general sharing), allocate+`CloneRecord` if not found, then recurse into that new object's own fields. Value-typed fields copy directly (no lookup).
- [ ] 3.3 Confirm the synthetic-slot spilling from `fase-4e-colector-mark-sweep`'s D4 already covers every clone-produced managed reference (design D3 — no new rooting mechanism needed, but verify this is actually true for the specific instruction results this change introduces, don't just assume it from the design doc).
- [ ] 3.4 IR-level tests: `.clone()` on a shared-child graph produces the expected `CloneLookup`/`CloneRecord` sequence; a cyclic graph's lowering includes a self-referential `CloneLookup` hit; every clone-produced instruction result is included in the enclosing function's `gc_roots` (explicit test, not assumed — same rigor `fase-4e-weak`'s task 2.3 applied to `WeakUpgrade`'s roots).

## 4. Codegen (zirk-codegen-llvm)

- [ ] 4.1 Emit `CloneBegin`/`CloneEnd`/`CloneLookup`/`CloneRecord`/`CloneAlloc` as calls into new `zirk-runtime` entry points (task 5.1). Reuse the existing per-class field-offset walking codegen (whatever `apply_gc_path`/`gc_reference_paths` machinery already emits for mark) to drive the field-recursion loop, rather than duplicating a second field-offset walker.

## 5. Runtime (zirk-runtime)

- [ ] 5.1 `zirk_rt_clone_begin() -> *mut CloneCtx`, `zirk_rt_clone_lookup(ctx, source_addr) -> Option<*mut u8>` (or equivalent null-sentinel return, matching this codebase's existing FFI conventions), `zirk_rt_clone_record(ctx, source_addr, dest_addr)`, `zirk_rt_clone_end(ctx)`. The memoization table is a `zirk-runtime`-internal allocation (address-keyed, e.g. a hash map), outside the collector's own object header/mark/sweep machinery — same category as the frame stack itself, not collector-tracked.
- [ ] 5.2 Runtime unit tests: a memoization table correctly returns "not found" for a fresh context and "found" after a record; recording twice for the same source address is idempotent or well-defined (decide and document which, then test it).

## 6. Cross-cutting correctness tests (do not skip — same standard as the collector/Weak<T> work)

- [ ] 6.1 End-to-end `.zrk` fixture: clone a simple acyclic object graph (a class with a couple of scalar fields) — clone has independent identity (`is` false against source), equal field values.
- [ ] 6.2 End-to-end `.zrk` fixture: the sharing invariant from the spec itself — `a.left is a.right` before cloning (both point at the same child); after `mut b = a.clone()`, confirm `b.left is b.right` is `true` and `b.left is a.left` is `false`. This is the single most important test in this change — it is the concrete contract the spec states by name.
- [ ] 6.3 End-to-end `.zrk` fixture: a cyclic graph (e.g. a node whose field points back to itself, or a two-node cycle) clones without stack overflow or infinite loop, and the cloned cycle points within the new graph, not back into the source graph.
- [ ] 6.4 End-to-end `.zrk` fixture: mutating the clone does not affect the source (and vice versa) — the "never retains an alias to a cloned mutable source node" guarantee, observed concretely.
- [ ] 6.5 End-to-end `.zrk` fixture: force a collection mid-clone (`ZIRK_GC_THRESHOLD=1`, the same mechanism prior collector/Weak<T> e2e tests use) on a graph large enough that cloning crosses the threshold partway through — confirm the finished clone is fully correct (nothing partially-built got collected). This is the concrete test for design's own Goal about collector-safety during the traversal — do not skip it or treat design's reasoning as sufficient without an executed test.
- [ ] 6.6 Compile-time rejection fixture: a class with a `Pointer<T>` (or `Resource`/`Task<T>`) field attempting `.clone()` fails to compile with a diagnostic naming that field.
- [ ] 6.7 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace` (multiple times, watching for flakiness the way the `Weak<T>` change found a real one — do not assume a single green run is sufficient given this change also introduces new global/shared runtime state); `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 7. Documentation and status sync

- [ ] 7.1 Update `docs/init/ZIRK_ROADMAP.md` Phase 4e's "deep clone graph semantics" bullet — delivered.
- [ ] 7.2 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md`'s memory rows (`deep clone()` moves from "remaining" to delivered).
- [ ] 7.3 Check `docs/handbook` and `docs/01_plantilla_zirk.md`/`docs/MEMORY_AND_UNSAFE_SEMANTICS.md` for any existing `Clone`/`clone()` teaching material with a stale caveat or syntax differing from the actual implementation — reconcile.
- [ ] 7.4 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 7.5 Report both the zirk-lang and zirk-lang-site revisions used.

## 8. OpenSpec close-out

- [ ] 8.1 Run `openspec validate fase-4e-clone` before archiving.
- [ ] 8.2 Archive the change once implementation, tests, and documentation sync are complete.
