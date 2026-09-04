## 1. Runtime string/char allocation through the GC

- [x] 1.1 Add `zirk_rt_string_descriptor` static and accessor in `crates/zirk-runtime/src/collector.rs`, with `gc_field_count` set to zero so the collector does not follow `ZirkString.bytes` as a reference.
- [x] 1.2 Update `ZirkString` overlay and `borrow()` in `crates/zirk-runtime/src/string.rs` to read the payload at `object + HEADER_BYTES`.
- [x] 1.3 Introduce allocation helpers that call `zirk_rt_alloc` for a string object: one for literal/external bytes and one for owned inline bytes, writing descriptor, `bytes`, `len`, and `is_ascii`.
- [x] 1.4 Port `zirk_str_from_utf8`, `zirk_str_from_i8/i16/i32/i64/i128`, `zirk_str_from_u8/u16/u32/u64/u128`, `zirk_str_from_bool`, `zirk_str_from_f32/f64`, `zirk_str_concat`, `zirk_str_repeat`, and `zirk_str_grapheme_slice` to the new allocation path without leaking `Box` allocations.

## 2. Compiler root enumeration for `String` and `Char`

- [x] 2.1 Add `IrType::String` and `IrType::Char` to `is_managed_reference()` in `crates/zirk-ir/src/ir.rs` so synthetic-slot spilling covers them.
- [x] 2.2 Add `IrType::String` and `IrType::Char` as leaf managed-reference cases in `gc_reference_paths()` in `crates/zirk-codegen-llvm/src/emit.rs` so class/record/value fields and `Nullable<String?`/`Char?` payloads are rooted.

## 3. Deep clone interaction with immutable strings

- [x] 3.1 Update `clone_recursive()` in `crates/zirk-runtime/src/clone.rs` to detect the string descriptor and return the source handle unchanged, sharing immutable `String`/`Char` objects instead of copying the absolute `bytes` pointer incorrectly.
- [x] 3.2 Add runtime tests for cloning a class with `String` fields and for cloning a graph that contains a cycle through a shared string.

## 4. Overflow-checked `++` and `--`

- [x] 4.1 Generalize `check_increment()` in `crates/zirk-sema/src/checker.rs` to accept any `Base::Int(_)` width and signedness, not only `Int32`.
- [x] 4.2 Rewrite `lower_increment()` in `crates/zirk-ir/src/lower.rs` to emit `const_int_at(1, ty, span)` and call `emit_checked_binary()` with `Add` for `++` and `Sub` for `--`, so prefix and postfix forms throw `ArithmeticOverflowError` on overflow.
- [x] 4.3 Add `zirk-ir/tests/lowering.rs` cases verifying `i++` at `Int32.MAX`, `--i` at `UInt8.MIN`, and an `Int16` postfix increment overflow.
- [x] 4.4 Add CLI corpus fixtures for `++`/`--` overflow in expression position and matching `.out` expectations.

## 5. Memory and integration tests

- [ ] 5.1 Add a corpus fixture that loops building and discarding strings and assert a stable peak RSS in `crates/zirk-cli/tests/end_to_end.rs` or a dedicated runtime test.
- [ ] 5.2 Add corpus fixtures for `String + String`, `String * n`, `String[index]`, and `String.grapheme_slice` in loops.
- [x] 5.3 Run `cargo test --workspace` and fix all failures introduced by root-enumeration or string-allocation changes.
- [x] 5.4 Run `cargo clippy --workspace` and resolve any new warnings.
- [x] 5.5 Run `openspec validate --all --strict` and fix any validation failures.

## 6. Public documentation and site synchronization

- [x] 6.1 Update `docs/init/ZIRK_FEATURE_STATUS.md` to mark the `Non-moving mark-sweep GC` row and any `String`/`Char`-related notes as completed for this change.
- [x] 6.2 Update `docs/handbook/13-appendices/07-current-limitations.md` to remove or rephrase any wording implying `String`/`Char` values are not reclaimed.
- [x] 6.3 Update `docs/decisions/proximos-pasos-fase-4.md` section 6.1 to record the decision that `String`/`Char` lifetimes are managed by the same collector as ordinary objects.
- [x] 6.4 Run `./scripts/sync-website-content.sh` and review the `../zirk-lang-site` diff.
- [x] 6.5 If project-status evidence changed, review the zirk-lang-site status catalog and pass an explicit `--audit-date YYYY-MM-DD`.
