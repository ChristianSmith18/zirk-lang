# ADR-014 — `Char` shares `String`'s opaque representation

- **Status:** accepted
- **Date:** August 19, 2026
- **Phase:** 3b

## Context

`ZIRK_LANGUAGE_SPEC.md` section 3 defines `Char` as "exactly one Unicode grapheme, even when it is composed of multiple code points and bytes". An extended grapheme cluster (a base plus combining marks, or a ZWJ-joined emoji sequence like `'👨‍👩‍👧‍👦'`) has no maximum size guaranteed by the Unicode standard — it can arbitrarily exceed 4 UTF-8 bytes.

`openspec/changes/fase-3b-scalars-and-text/design.md` left this as an open question with three options: (a) an opaque view backed by the runtime, like `String`; (b) an inline value with small-buffer optimization and an escape path for the rare case that exceeds the buffer; (c) restrict `Char` to a single code point, which contradicts section 3 as written and would need a spec correction first.

## Decision

`Char` is represented exactly like `String` ([ADR-005](./ADR-005-representacion-string.md)): an opaque handle to the same kind of UTF-8 content that the runtime already knows how to build, compare, and free. The IR gains its own `IrType::Char` — it does not reuse `IrType::String` — so the checker and the verifier can keep statically distinguishing "this is exactly one grapheme" from "this is arbitrary text", but in LLVM both lower to the same opaque pointer and the same `extern "C"` symbols (`zirk_str_from_utf8` to construct, `zirk_str_eq` for `==`).

Checking that a `'...'` literal contains exactly one extended grapheme (UAX #29) happens in `zirk-sema`, not in the lexer or the runtime: the lexer already documented that boundary (in `zirk-lexer::character()`'s own comment), and the runtime doesn't need to re-segment a value the compiler already validated at the single point where a `Char` is built from literal text.

## Rationale

- **Option (a) over (b):** an inline buffer with an escape path duplicates exactly the machinery `String` already has — allocation when it doesn't fit inline, freeing, content comparison — for an unmeasured performance gain, and one that only applies to the common case (graphemes made of one base plus a few marks), without avoiding the rare case (a long ZWJ sequence) that still needs the escape path anyway. The complexity gets paid twice: once in the new runtime machinery, again in codegen, which now needs to know when a `Char` is inline and when it isn't. Option (a) pays the complexity only once, already paid by `String`.
- **Option (c) discarded** because it requires correcting the spec before implementing, and the spec is already explicit and unambiguous in section 3 — there's no reasonable alternative reading that would justify reopening it merely for implementation convenience.
- **Its own `IrType::Char`, instead of reusing `IrType::String` directly:** although the runtime representation is identical, they are observably distinct types for a Zirk program — `Char` doesn't have `String`'s mutation methods, and (unlike `String`, see ADR-005's amendment) `Char` has no observable identity: `is` is rejected on `Char` because it's a value, not a reference with shared-alias semantics, even though its low-level representation is a pointer. Collapsing both into a single `IrType` would force encoding that distinction elsewhere (a flag, an exception list) instead of letting the type itself carry it.

## Consequences

- No new runtime: `Char` reuses `zirk_str_from_utf8`/`zirk_str_eq` as-is.
- A `Char` is allocated the same way as a `String` (`IrType::needs_allocation` is true for both) — the cost of a `Char` is no different from that of a one-grapheme `String`, which is honest given that the representation is the same.
- If a later phase measures that the indirection of a single-ASCII-code-point `Char` is a real cost in a concrete case, it gets optimized there with evidence — the same closure principle ADR-005 already adopted for `String`.
- Comparison (`<`, `<=`, …) and any grapheme-specific operation (uppercase/lowercase, Unicode category) are outside this decision's scope: they are library surface (`ZIRK_STDLIB_SPEC.md`), not representation.
