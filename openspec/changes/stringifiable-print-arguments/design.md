## Context

Before this change the checker special-cased standard stream calls: it accepted any printable value, the lowering rendered every argument to a `String` and passed a `List<String>` to the bundled body, and the body only joined and wrote. `to_string()` was a universal member supplied by the compiler for built-in families and derived for user types, with no trait behind it. See `proposal.md` for motivation.

## Goals / Non-Goals

**Goals:**
- One checkable standard contract for "can be rendered as text".
- Stream functions that are ordinary Zirk code calling `arg.to_string()`.
- No change to rendering rules (`universal-string-rendering` keeps owning them).

**Non-Goals:**
- Making `Stringifiable` user-visible syntax beyond `#override`; classes do not need to write `implements Stringifiable`.
- Changing `Void`/`Never` handling or the `<Null>` rendering.
- Moving string interpolation onto the trait.

## Decisions

1. **A real trait in `std.io`, recognized by provenance.** `Stringifiable` is declared in the bundled `package.zrk`; the checker records its contract id when it checks the declaration from that source (`native_stringifiable`). A user trait with the same name is never mistaken for it. Alternative considered: a hard-coded builtin contract — rejected because the standard library should be able to state its own contract in Zirk.
2. **Satisfaction is automatic for value-producing types.** Assignability to `Stringifiable` delegates to the existing printable-value check, so scalars, collections, records, enums, classes and nullable values all qualify without declarations. Alternative considered: requiring `implements Stringifiable` — rejected because it would break every existing class and string interpolation parity.
3. **The trait's default body is concrete, hence `#override`.** The default returns a placeholder; real rendering is the compiler default for built-in families and derived text for user types. A class `to_string()` replaces a concrete body, so the existing "replaces a default" rule applies uniformly. The marker is treated as necessary (not "unnecessary") only for `to_string`.
4. **Output goes through two private leaves.** `write(text)` and `write_line()` replace the previous single `write_line(text)`; lowering maps them to `Print`/`Println` of the text or an empty string. This keeps the runtime boundary at single-text writes.
5. **Logical output events are lines, not writes.** Because a call now produces several writes, the interpreter counts an output event only when a write starts a new line on its stream, and the captured host merges consecutive pieces of one line. Alternative considered: coalescing in the lowering with a buffer — rejected because it changes where output ordering between streams is decided and hides partial output on failure.

## Risks / Trade-offs

- [Existing classes with `to_string()` stop compiling] → Diagnostic `MISSING_OVERRIDE` names the fix; corpus and handbook updated; release note in the changelog of the language spec.
- [The default body's `"<Value>"` placeholder could surface] → It is reachable only for a value with no compiler rendering, which the checker rejects; a test covers `Void`/`Never` rejection.
- [More writes per `println` slow interpreted output] → Cost is a few extra instructions per argument; output byte and event limits are unchanged in meaning.
- [Merged events lose the original write boundaries] → Boundaries within a line were never part of the contract; ordering and total text are preserved.
