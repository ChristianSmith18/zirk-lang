## 1. Standard library and checker

- [x] 1.1 Declare `Stringifiable` in `stdlib/std/io/package.zrk` and change the four stream functions to `...args: Stringifiable`; verify `cargo test -p zirk-parser` accepts the bundled sources.
- [x] 1.2 Record the bundled trait by provenance in the checker and check output arguments against it, accepting nullable values; verify `crates/zirk-sema/tests/typing.rs` covers mixed arguments.
- [x] 1.3 Require `#override` for a class `to_string()` and stop flagging it as unnecessary; verify the diagnostic test and the updated corpus programs.

## 2. Lowering and runtime boundary

- [x] 2.1 Replace the single `write_line(text)` leaf with `write(text)` and `write_line()` and lower them to `Print`/`Println`; verify `crates/zirk-ir/tests/lowering.rs`.
- [x] 2.2 Update the corpus programs `to_string_contract`, `to_string_followups` and `universal_to_string` and the parser grammar tests; verify `cargo test --workspace --no-fail-fast` has no failures.

## 3. Browser interpreter

- [x] 3.1 Count output events per logical line and merge pieces of a line in `CapturedHost`; verify the interpreter unit test and the browser contract tests.
- [x] 3.2 Add the `std-io-mixed-values` fixture with native-derived output; verify the fixture runners agree.

## 4. Documentation and companion-site handoff

- [x] 4.1 Update `ZIRK_LANGUAGE_SPEC.md`, `ZIRK_STDLIB_SPEC.md` and the `std.io` handbook chapter; verify the examples match `examples/std_io_mixed_values.zrk`.
- [x] 4.2 Run `./scripts/sync-website-content.sh`, obtain human review of the `zirk-lang-site` status catalog, and pass an explicit `--audit-date YYYY-MM-DD`.
- [x] 4.3 In `../zirk-lang-site`, adopt the synced content and the rebuilt package, review the diff, and commit it separately with both revisions recorded.
