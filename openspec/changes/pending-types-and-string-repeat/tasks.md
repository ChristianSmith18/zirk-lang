# Tasks

## 1. `UInt` alias

- [x] 1.1 In `crates/zirk-sema/src/types.rs`, make `UInt` (and `UInteger`) resolve to `Type::UINT32` in `Type::from_name` / the pending-name gate, removing the "arrives in Phase 3b" rejection for `UInt`.
- [x] 1.2 Add `zirk-sema` typing tests: `UInt` annotation accepts a literal, reports the same range as `UInt32`, and rejects a negative literal the same way `UInt32` does.
- [x] 1.3 Search docs for `UInt` listed as pending/not-implemented and update them (`ZIRK_FEATURE_STATUS.md`, roadmap, handbook type pages).

## 2. `String * Int` inside larger expressions

- [x] 2.1 Add a failing IR lowering test: `"x: " + "ab" * 3` (and a call-argument variant) currently produces invalid IR.
- [x] 2.2 Fix `opens_blocks` in `crates/zirk-ir/src/lower.rs` so `Mul` over `String` (i.e. the `checked_repeat` path) counts as block-opening; re-run the new test.
- [x] 2.3 Add a CLI valid-corpus fixture `string_repeat_nested.zrk` exercising `String * Int` inside concatenation, call arguments and `println`, with its `.out`.

## 3. `unsafe`/`Pointer` documentation and example

- [x] 3.1 Handbook coverage of `unsafe`/`Pointer`/`NativeSlice` — verified already complete under `17-memory-and-safety/` (pointers, dereferencing, unsafe blocks, transactional commit pages)
- [x] 3.2 `ZIRK_FEATURE_STATUS.md` — verified already reports `unsafe`/`Pointer`/`NativeSlice` as shipped (Phase 4e table)
- [x] 3.3 Add a `UInt` and an `unsafe`/`Pointer` section to the `hello.zrk` everything-example.

## 4. Verification and site

- [x] 4.1 `cargo fmt --check`, `cargo test --workspace`, `cargo test -p zirk-cli --test end_to_end`.
- [ ] 4.2 Run `./scripts/sync-website-content.sh --audit-date <today>` and commit the companion `zirk-lang-site` changes separately.
