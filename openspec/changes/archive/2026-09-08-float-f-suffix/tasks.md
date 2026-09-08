# Tasks: float-f-suffix

## 1. Lexer

- [x] 1.1 Change `FLOAT_WIDTHS` in `crates/zirk-lexer/src/token.rs` to `["f", "f16", "f32", "f64", "f128"]` and update the doc comments that cite `b`/`bN` spellings.
- [x] 1.2 In `crates/zirk-lexer/src/lib.rs` suffix handling, recognize `b`/`b16`/`b32`/`b64`/`b128` explicitly and emit a diagnostic stating the binary-float suffix is now `f`/`fN`; keep `d` as the `Duration` days unit and let other unknown suffixes take the generic invalid-suffix path (which must not mention `b` or `d` as valid spellings).
- [x] 1.3 Update `crates/zirk-lexer` tests (`1.5b32` → `1.5f32`, etc.) and add cases: `1.5f` → `Float64`, `0.1f128` → `Float128`, `1.5b` rejected with the `f` pointer, `1.5d` still lexes as a `Duration` of days.

## 2. Semantic checker

- [x] 2.1 In `check_float_literal` (`crates/zirk-sema/src/checker.rs`), map `f`/`f64` → `F64`, `f16` → `F16`, `f32` → `F32`, `f128` → `F128`; update comments and any diagnostic that suggests a `b` suffix to say `f`.
- [x] 2.2 Grep the whole workspace for `b*`-suffix literal spellings in code, tests, and diagnostics (e.g. `1.5b`, `0.0b`, `b32`) and switch them to `f*`; verify no diagnostic recommends `b` or `d`.
- [x] 2.3 Update `zirk-sema`/`zirk-ir`/`zirk-parser`/`zirk-codegen-llvm` tests that use `b*` literals to `f*` spellings.

## 3. Verification

- [x] 3.1 `cargo test -p zirk-lexer -p zirk-sema -p zirk-parser -p zirk-ir -p zirk-codegen-llvm` passes.
- [x] 3.2 Compile a smoke program using `1.5f`, `1.5f32`, `1.5` (Decimal), and `1.5d` (Duration of days), and confirm `1.5b` produces the specified diagnostic.

## 4. Docs

- [x] 4.1 Update remaining `b`-suffix mentions in `docs/init/*`, `docs/ZIRK_*_SPEC.md`, and other internal docs to `f`/`fN`; confirm no doc presents `d` as a decimal suffix.
- [x] 4.2 If any public docs changed after commit, run `./scripts/sync-website-content.sh` per AGENTS.md.
