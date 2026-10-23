## Why

`print` and `println` were declared as `...args: String` in the bundled `std.io` source, while the checker accepted every printable value through a special rule and the compiler rendered the arguments before the Zirk body ran. The standard library therefore could not express its own contract: the public signature lied about what callers may pass, and the Zirk body never invoked `to_string()` itself. Making `Stringifiable` a real standard trait lets the stream functions be ordinary variadic Zirk functions that call `arg.to_string()` and delegate only single-text writes to the runtime.

## What Changes

- Add the standard trait `Stringifiable` to `std.io`, with a concrete default `to_string()` body. Every value-producing type satisfies it automatically; `Void` and `Never` still have no text form.
- Change `stdout.print`, `stdout.println`, `stderr.print` and `stderr.println` to `...args: Stringifiable`. Their bundled bodies join the rendered arguments with one space and write each piece through private `write` and `write_line` leaves; the compiler no longer pre-renders the argument list.
- The checker checks each output argument against `Stringifiable` instead of a printable-value shortcut. Nullable values stay accepted and render `<Null>`.
- A class that declares `to_string()` replaces the trait's concrete default and must mark it `#override`; the marker is no longer reported as unnecessary for `to_string`.
- **BREAKING (source):** existing classes with a `to_string()` method need `#override`. The corpus programs `to_string_contract`, `to_string_followups` and `universal_to_string` were updated accordingly.
- The interpreter treats writes to one unfinished line as a single logical output event: the event allowance counts lines, and the captured host merges the pieces, so consumers still see one event per line.
- This change affects normative language and standard-library text and the handbook, so public content changes.

## Capabilities

### New Capabilities

### Modified Capabilities

- `universal-string-rendering`: the universal `to_string()` contract is expressed through the `Stringifiable` trait, including the `#override` rule for classes.
- `zirk-standard-library`: output functions accept heterogeneous `Stringifiable` arguments.
- `zirk-ir-interpreter`: output pieces of one line form one logical event and are counted once.

## Impact

- `stdlib/std/io/{package,stdout,stderr}.zrk`, `crates/zirk-sema/src/{checker,stdlib}.rs`, `crates/zirk-ir/src/lower.rs` and their tests.
- `crates/zirk-interpreter/src/lib.rs` (`emit_output`, `CapturedHost::emit`) and the browser fixtures.
- Corpus programs with a user-written `to_string()` and the parser grammar tests that described the old compatibility lowering.
- `docs/ZIRK_LANGUAGE_SPEC.md`, `docs/ZIRK_STDLIB_SPEC.md`, `docs/handbook/04-standard-library/02-std-io.md`.
- Companion repository `../zirk-lang-site` is impacted: the handbook and normative spec changed, so run `./scripts/sync-website-content.sh` after the commit, review the site status catalog, and pass an explicit `--audit-date`.
