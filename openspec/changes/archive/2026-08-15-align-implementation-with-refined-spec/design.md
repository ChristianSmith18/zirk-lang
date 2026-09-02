## Context

Phases 0-2 were built against an earlier version of the documentation. The
subsequent normative refinement removed the `Decimal` family, defined `Char`
as a Unicode grapheme, fixed `String` as a shared mutable reference indexed
by graphemes, and introduced the temporal family, deep contextual
conversion, and `inmut::strict` with alias analysis.

An audit of the code against the current normative sources yields an
asymmetric result:

- **The implemented semantics are correct.** Truncated-toward-zero division
  and remainder with the sign of the dividend come from `sdiv`/`srem`;
  overflow and division by zero are controlled errors via LLVM intrinsics,
  including `-Int32.MIN` and `Int32.MIN / -1`; there is no truthiness or
  implicit conversions; `T?` is a bit over the base and `null` only inhabits
  nullable types. None of this needs to be touched.
- **The lexical surface is misaligned.** The lexer does not recognize
  literals that the language has, and the pending type table advertises
  types the language no longer has.

The most serious case is not a wrong diagnostic but a missing one: `1.5` is
tokenized as `1`, `.`, `5` without saying anything. The lexer already has the
correct discipline for this -- recognizing constructs from later phases in
order to defer them by name and phase -- and this change extends it to what
was left out.

In addition, several documented features do not appear in any roadmap phase.
As long as that remains true, Phase 3 has an open door to absorb them, which
is exactly what its own scope prohibits.

## Goals / Non-Goals

**Goals:**

- Have the compiler recognize the entire lexical vocabulary of the language
  and defer, by name and phase, what it does not implement.
- Make the statement forms that the standard defines and the parser never
  implemented -- optional parentheses, `do ... while`, brace-free `if`,
  ternary, increment as expression -- compile.
- Have the pending type table describe the language as it currently stands:
  no `Decimal`, with `Float`, and with the temporal family.
- Enable the `Int` and `Integer` aliases, which name a type already
  implemented.
- Fix `String` identity, equality, and hash with an implementation that does
  not pay for normalization in the common case.
- Give an owning phase to every documented feature that currently has none.
- Amend ADR-005 and record the decision about normalization.

**Non-Goals:**

- Implementing `Float` arithmetic, `Char`, the temporal family,
  `inmut::strict`, interpolation, or the bitwise operators. This change makes
  them visible to the compiler, it does not build them.
- Modifying Phase 3's scope.
- Rewriting `String`'s internal representation or building the grapheme
  index. That is brought by the phase that indexes it.
- Changing the behavior of any program that compiles and runs today. The set
  of valid programs is neither reduced nor expanded; only what is said about
  invalid ones improves.

## Decisions

### D1 -- The lexer recognizes the whole language; the parser defers what is missing

Lexical recognition and a feature's availability are different things, and
conflating them is what produces the diagnostic "unrecognized character: does
not start any token of the language" for `'`, a false statement.

Each new literal, operator, and keyword is lexed the same way as `class` or
`task`: it becomes a token, and the layer that knows about phases is the one
that rejects it.

The alternative -- recognizing them only once implemented -- is what produced
the current situation: every documented but unlexed feature is a promise of
a bad diagnostic.

### D2 -- A Float literal is deferred in the checker, not in the lexer

The lexer produces the literal with its value and its width if it has one.
The phase diagnostic is emitted by whoever knows about types.

That way, when Phase 3b implements `Float`, the lexer does not change: only
a rejection is removed. And meanwhile the token exists, which is what
prevents the silent reinterpretation of `1.5`.

### D3 -- Interpolation is lexed as structure, not as text

An interpolated string literal produces literal parts and embedded
expressions as distinct elements, with balanced braces. Keeping the braces
inside the text would force re-lexing the string later on, and the starting
point -- which file offset each expression corresponds to -- would already
have been lost for diagnostics.

The balancing is the same one `0..{number}` needs, so it is a single
mechanism.

### D4 -- `Int` and `Integer` are enabled; `UInt` is not

An alias is exactly its target. `Int` and `Integer` are `Int32`, which has
been implemented since Phase 1: blocking them was blocking a name, not a
capability. `UInt` is `UInt32`, which is not implemented, so it remains
deferred together with it.

The resolution happens in the name-to-type mapping, not by creating new
bases: after resolution, nothing distinguishes an `Int` from an `Int32`,
which is exactly what being an alias means.

### D5 -- `is` compares handles; `==` compares canonical content

The observable rule is the one the author requested: `"Å" == "Å"` is
`true` even if one is in NFC and the other in NFD, and `is` distinguishes
referents.

The implementation avoids the cost in the common case, in this order:

1. Same handle -> equal. This is also `is`'s answer.
2. Identical lengths and bytes -> equal. A `memcmp`, without allocating
   memory.
3. Both marked canonical and with different bytes -> different.
4. Only then, incremental canonical comparison, without materializing
   normalized copies when avoidable.

The handle stores, alongside the bytes, flags and cached fields with English
names like the rest of the code: `is_ascii`, `normalization`,
`grapheme_count`, `hash`. An ASCII string cannot have distinct equivalent
forms, so `is_ascii` always short-circuits at step 2.

The hash is derived from the canonical form: if it were derived from the
bytes, two keys equal by `==` would fall into different buckets and
`Map<String, _>` would contradict the operator.

### D6 -- Literals are normalized at compile time

The compiler emits literals already in canonical form and marked as such.
This is work done once, on a machine that is not in a hurry, and it turns
the comparison between literals -- the overwhelmingly most common case --
into a byte comparison.

The alternative, normalizing at runtime, pays at every comparison a cost the
source already knew about.

### D7 -- ADR-005 is amended, not replaced

The new standard does not contradict the handle's opacity: it confirms it.
The handle is the identity that `is` compares, and being opaque is what
allows graphemes, normalization, and the cache to live entirely inside the
runtime without the compiler needing to know about them.

The amendment records both things: that the handle is observable identity,
and that equality and hashing are the runtime's responsibility.

### D8 -- Phase 3b is introduced instead of stretching Phase 3

The complete integer widths, the `Float` family, `Char`, deep contextual
conversion, the bitwise operators, and interpolation form a mutually
dependent block: deep contextual conversion means nothing without `Float`,
and interpolation needs the `to_string()` that Phase 3 turns into a
contract.

Putting them into Phase 3 would duplicate a phase that is already the
largest in the roadmap. Leaving them without a phase would turn them into
undated debt. A phase of its own, immediately after, is what respects both
things.

`inmut::strict` does not go there: it requires alias analysis, which is the
same analysis the memory model needs, and that is why it goes to Phase 4.
The temporal family goes to Phase 7, annotated as native types known by the
compiler and not as library objects -- the distinction matters because the
lexer and the checker know them before `std.time` exists.

### D9 -- The set of valid programs only grows

Everything this change adds applies to source that either does not compile
today, or compiles producing something different from what was written. The
edge case is `1.5`, which today is lexed as three tokens: it does not form
any valid expression of the subset, so no program that compiles depends on
that reading.

The statement forms D10 through D12 add are additions, not replacements:
`for (…)` remains valid. No program that compiles today stops doing so or
changes behavior.

Validation follows directly from that: the existing test suite passes
unmodified, except for the tests that explicitly pinned down the behavior
being corrected.

### D10 -- The header parentheses are optional, and that has to be written down

The parser requires parentheses in the traditional `for`. The standard
writes it without them. It is not that one source is wrong: it is that **the
real language rule was not written down anywhere**, and faced with that
silence the implementer assumed what they knew from C.

The rule is that the header of any control structure -- `if`, `while`,
`for`, `for ... in`, `do ... while`, `match` -- admits optional parentheses,
and that the canonical form omits them.

For `if`, `while`, and `match` this already works without touching anything:
`(cond)` is a parenthesized expression and the parser does not distinguish.
The real work is in the traditional `for`, whose header has three parts and
currently requires the delimiter.

Without parentheses, what closes the header is the block's `{`. Zirk has no
brace literals in expression position -- records are built as `Type(...)` --
so the ambiguity that forces other languages to prohibit this form does not
appear.

The rule is written into `ZIRK_LANGUAGE_SPEC.md` §5 and as a `zirk-grammar`
requirement. Writing it down matters more than implementing it: it is the
absence of the rule, not its content, that produced the divergence.

### D11 -- `do ... while`, the brace-free `if`, and the ternary are implemented here

All three are pure grammar: they introduce no new types, contracts, or
mutability rules. `do ... while` is a loop whose condition is evaluated at
the end; the brace-free `if` governs exactly one statement; the ternary is
an expression with the branches the `if` expression already knows how to
unify.

They enter this change because the parser has to be opened anyway for D10,
and a second pass over the same grammar would cost more than doing it
together.

The brace-free `if` governs **one** statement and does not admit `else`:
with `else` the *dangling else* ambiguity would return, and the standard
presents it as an effect form (`if closed return;`), not as a full
conditional.

### D12 -- Increment becomes an expression, because its reason has expired

Phase 2 deferred `++` and `--` as expressions with this reason, written in
the parser: *their prefix/postfix distinction would need an evaluation
order that no normative document defines*.

It was correct then. It no longer is: `ZIRK_LANGUAGE_SPEC.md` §4 requires
preserving the conventional postfix and prefix semantics, which is exactly
the order that was missing. `count++` evaluates to the previous value and
increments; `++count` increments and evaluates to the new one.

Also, D10 makes it unavoidable: the standard's canonical `for` is
`for mut i = 0; i < 10; i++ { }`, and its step is precisely an increment.

Both forms require an assignable and mutable place, the same condition that
compound assignment already checks.

## Risks / Trade-offs

- **[Recognizing without implementing could be read as a promise]** -> The
  diagnostic names the phase, not a date. This is the same convention the
  lexer has used since Phase 1 with `class` and `task`, and no one has read
  it as a deadline promise.

- **[Canonical comparison requires Unicode data]** -> The common case does
  not touch it (D5, steps 1-3), and literals arrive normalized (D6). If the
  binary-size cost turned out to matter, the table can be trimmed to what
  canonical equivalence needs, which is much less than full Unicode.

- **[Phase 3b delays Phase 4]** -> Only on paper. That work has to be done
  either way; today it was hidden in a roadmap that did not name it, which
  is worse than being planned.

- **[Marking normalization state could get out of sync with the bytes]** ->
  The state is private to the runtime and is only set where the handle is
  built. No path that modifies bytes exists yet; when the phase that
  mutates `String` introduces one, invalidating the cached fields --
  `normalization`, `grapheme_count`, `hash` -- is part of that path and of
  its tests.

- **[The new statement forms do reach the binary]** -> This is the only part
  of the change that touches IR and codegen, and it breaks the "only the
  entry gate" cleanliness. It is bounded by the D11 rule: nothing that
  introduces types or contracts enters here. `do ... while` lowers to the
  same basic blocks as `while` with the initial jump inverted, and the
  ternary to the same ones as the `if` expression; neither introduces a new
  IR form.

- **[The brace-free `if` invites *dangling else*]** -> It does not admit
  `else` (D11). An `else` after a brace-free `if` is an error with its own
  diagnostic, not an ambiguity to resolve by precedence.

- **[Lexed interpolation without semantics leaves a structure without a
  consumer]** -> This is deliberate: the structure is produced by the lexer
  and consumed by Phase 3b. The cost is a live representation with no use;
  the benefit is that Phase 3b does not have to touch the lexer again.

## Migration Plan

1. Amend ADR-005 and record the decision on `String` identity, equality, and
   normalization. Decisions come before code.
2. Update `ZIRK_ROADMAP.md` with Phase 3b and the assignment of orphaned
   features. Without this, the phases the new tokens declare would have
   nothing to refer to.
3. Lexer: tokens and keywords, then literal recognizers.
4. Checker: pending type table, aliases, `for ... in` element.
5. Runtime: equality, hash, and normalization of literals.
6. Verify that the existing test suite passes unmodified, except for the
   tests that explicitly pinned down the corrected behavior.

There is no rollback to plan: the change does not alter persisted formats or
the generated binary. Reverting the commit is enough.

## Open Questions

- Is Phase 3b a phase of its own or a numbered second half of Phase 3? A
  phase of its own is proposed due to size; this is a roadmap decision, not
  a technical one, and it blocks nothing in this change. Phase 7b follows
  the same convention for the same reason: numbering it as 8 would shift
  phases 8 through 12, which have existed since the roadmap was written.
- Should the formatter **normalize** optional parentheses to the canonical
  form, or respect what the author wrote? D10's rule states which form is
  canonical, not whether the formatter enforces it. This is decided in
  Phase 9, which is the one that brings the formatter.
- Do width suffixes in literals (`1.5f32`) also admit an integer form
  (`42i64`)? `04-literals.md` mentions "suffix" for both families without
  fixing the integer spelling. What is documented is lexed, and the
  extension is left to Phase 3b, which is the one that needs it.
