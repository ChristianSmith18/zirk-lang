## 1. Checker (zirk-sema)

- [ ] 1.1 Add `Base::NativeSlice(u32)`/`Base::NativeSliceMut(u32)` (ids into an interning table, matching `Base::Pointer(u32)`'s actual current shape — verify, don't assume, per `fase-4e-weak`'s own precedent of checking first). Restrict element type `T` to the same ABI-safe subset `Pointer<T>` already accepts (verify the actual current list in the checker, not just design's restated summary of it).
- [ ] 1.2 Type `pointer.as_slice(length): Result<NativeSlice<T>, NativeError>` and `pointer.as_slice_mut(length): Result<NativeSliceMut<T>, NativeError>` as compiler-recognized methods on any `Pointer<T>`-typed receiver, requiring `unsafe` context (same treatment `Pointer<T>`'s other operations already have).
- [ ] 1.3 Type indexing (`view[i]`) and `.length: UInt`/`.is_empty: Boolean` on `NativeSlice<T>`/`NativeSliceMut<T>` — usable in ordinary safe code (no `unsafe` needed to read/write through an already-constructed view).
- [ ] 1.4 Escape checking (design D4): generalize `fase-4e-unsafe-pointer-extern`'s existing `Pointer<T>` escape check (rejects any occurrence as a return value, field-write source, or closure capture) to also match `NativeSlice<T>`/`NativeSliceMut<T>` — extend the existing pass/predicate, do not duplicate it. New diagnostic continues the `E0444`-range family the pointer-escape checks already use — confirm the actual next free code before assigning.
- [ ] 1.5 Checker unit tests: `as_slice`/`as_slice_mut` outside `unsafe` is rejected; element type outside the ABI-safe subset is rejected (matches the spec delta's "NativeSlice element type is ABI-safe only" scenario); indexing/`.length`/`.is_empty` type correctly; returning a `NativeSlice<T>` whose owner ends in the function is rejected (matches the existing "Native view escapes its borrow" scenario, now actually enforced); capturing a view in a closure is rejected.

## 2. IR (zirk-ir)

- [ ] 2.1 Define the view's IR representation as a `(pointer, length)` pair (design D2 — no allocation, no header). Add construction instructions (`SliceFromPointer`/`SliceMutFromPointer`, validating and producing the `Result`) and indexed access instructions with an explicit bounds check against the carried length (design D3 — reuse the exact bounds-check IR shape `Array<T>`/`List<T>` indexing already emits, don't invent a second one).
- [ ] 2.2 IR-level tests: construction lowers to a validation call producing a `Result`; indexing lowers to a bounds check followed by the actual load/store; a view value is never included in `gc_roots` (it's not a managed reference — confirm this explicitly, don't just assume it from design D2).

## 3. Codegen (zirk-codegen-llvm)

- [ ] 3.1 Emit view construction as a call into a new `zirk-runtime` validation helper (task 4.1) returning the validated `(pointer, length)` pair or an error.
- [ ] 3.2 Emit indexed access as: bounds check (compare index against the carried length, branch to a controlled bounds-error path on failure — reuse whatever `Array<T>`/`List<T>` indexing already emits for this, confirm the actual current codegen shape first) then a direct load/store at `pointer + index * sizeof(T)`.

## 4. Runtime (zirk-runtime)

- [ ] 4.1 Validation helpers for construction: non-null check, alignment check (against `T`'s known alignment), extent representability, and — where the underlying allocation is a `zirk_rt_alloc`'d buffer (its size is knowable via the object header) — a check that `length` doesn't exceed it. Where provenance isn't knowable (an opaque foreign pointer), validate only what's checkable and document that the caller-supplied `length` is trusted beyond that point (design's own accepted risk).
- [ ] 4.2 Runtime unit tests: valid construction succeeds; null pointer rejected; misaligned pointer rejected; a `length` exceeding a known `zirk_rt_alloc`'d buffer's real size rejected.

## 5. Cross-cutting correctness tests (do not skip)

- [ ] 5.1 End-to-end `.zrk` fixture: construct a `NativeSlice<T>`/`NativeSliceMut<T>` from a validated `Pointer<T>`, read/write through it in safe code (outside any `unsafe` block) — confirm this actually compiles and runs without requiring `unsafe` at the use site, only at construction.
- [ ] 5.2 End-to-end `.zrk` fixture: an out-of-range index on a constructed view fails with a controlled bounds error, not a crash or silent out-of-bounds access.
- [ ] 5.3 End-to-end `.zrk` fixture: `pointer.as_slice(length)` with a `length` that exceeds the real underlying allocation returns `Error`, not a view that would allow out-of-bounds reads.
- [ ] 5.4 Compile-time rejection fixture: returning a `NativeSlice<T>` whose owner is a `Pointer<T>` local to the function fails to compile — the "Native view escapes its borrow" scenario, exercised for real (this scenario was already normative but unenforced before this change; confirm it is now actually enforced, not just documented).
- [ ] 5.5 Ran `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20 cargo test --workspace`; `cargo clippy --workspace --all-targets` clean; `cargo fmt --check` clean.

## 6. Documentation and status sync

- [ ] 6.1 Update `docs/handbook/13-appendices/07-current-limitations.md` and `docs/handbook/11-reference/12-feature-status.md`'s memory rows — `NativeSlice<T>`/`NativeSliceMut<T>` delivered (volatile access remains open, unchanged, on `Pointer<T>` only).
- [ ] 6.2 Reconcile `docs/handbook/05-native-and-low-level/02-importing-c.md` and `03-exporting-zirk.md` (both already show `NativeSlice<T>` usage ahead of implementation) against the real implemented surface.
- [ ] 6.3 Commit the zirk-lang changes, then run `./scripts/sync-website-content.sh` from the repo root (with `--audit-date YYYY-MM-DD` using the actual date if project-status evidence changed).
- [ ] 6.4 Report both the zirk-lang and zirk-lang-site revisions used.

## 7. OpenSpec close-out

- [ ] 7.1 Run `openspec validate fase-4e-native-slice` before archiving.
- [ ] 7.2 Archive the change once implementation, tests, and documentation sync are complete.
