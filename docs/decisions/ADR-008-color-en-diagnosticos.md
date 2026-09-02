# ADR-008 — Color in diagnostics

- **Status:** accepted
- **Date:** August 14, 2026
- **Phase:** 1

## Context

`ZIRK_COMPILER_SPEC.md` section 8 defines the **format** of a diagnostic — severity, code, location, cause, help — but says nothing about its presentation. Color doesn't change the format: it changes how costly it is to read.

Section 9 of the same document does impose a restriction that conditions how it is implemented:

> The CLI must start fast, produce **deterministic output** and offer a structured mode (`--json`) for tooling.

A colored diagnostic that reaches a pipe is not deterministic: the tool consuming it receives escape sequences it didn't ask for.

## Decision

**Diagnostics are colored only when the destination is a terminal being read by a person.**

The library never decides on its own: `render()` carries no color and `render_colored(Color)` does, so whoever renders declares the destination. The CLI resolves the color with this precedence, from strongest to weakest:

1. `--color=always` or `--color=never` on the command line;
2. the `NO_COLOR` variable, which by convention disables color regardless of its value ([no-color.org](https://no-color.org));
3. whether the error output is a terminal.

That an explicit flag wins over `NO_COLOR` is what `rustc`, `cargo` and `ripgrep` do: whoever writes it is requesting it for that specific invocation.

**The structured form never carries color**, even if requested: an escape sequence inside a JSON string breaks whoever parses it.

### Palette

Roles are named after what they mean, not their color, so that a future theme can change the palette without touching the rendering.

| Role | Color | Why |
|---|---|---|
| Error | muted red, bold | Basic ANSI red is aggressive in most themes, and a diagnostic that shouts reads worse |
| Warning | amber | Clearly distinct from the error's red |
| Location and channel | gray | They matter, but they aren't the first thing read |
| Highlighted fragment | **bold only** | The source retains its own readability; the emphasis marks the exact point |
| `^^^` marker | same red as the error | Visually ties the marker to the header |
| `= cause:` | soft blue | |
| `= help:` | soft violet | Distinguishing it from the cause matters: they answer different questions |

## Rationale

Color is implemented with hand-written ANSI sequences, with no dependency. The whole compiler has a single external dependency — `inkwell` — and it's there because hand-writing LLVM bindings isn't reasonable. Color is a handful of escape sequences; adding a crate would trade a real cost — one more thing to audit, version and keep working on three platforms — for very little.

Terminal detection uses `std::io::IsTerminal`, which is from the standard library.

## Consequences

- **Redirected output is byte-for-byte identical to what it was before this change.** There is a test that compares both after stripping escape sequences, because the property that matters is that color adds emphasis without changing what is said.
- Existing tests were left untouched: they use `render()`, which remains colorless. That is evidence the change is purely additive.
- The emphasis of the highlighted fragment splits on Unicode characters and not on bytes: cutting an `ñ` in half would corrupt the output.
- On Windows, color depends on the terminal processing ANSI sequences. Modern terminals do; on one that doesn't, TTY detection would still return true and the raw codes would show up. This wasn't observed in practice and will be revisited if it appears.
