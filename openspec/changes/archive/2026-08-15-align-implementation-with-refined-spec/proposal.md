## Why

The normative documentation was refined after Phases 0-2 had already been
implemented: the `Decimal` family disappeared in favor of `Float`, `Char`
became a Unicode grapheme, `String` was defined as a shared mutable reference
indexed by graphemes, and the temporal family, deep contextual conversion, and
`inmut::strict` with alias analysis appeared.

The compiler still knows nothing about any of that. Its **implemented
semantics are correct** -- truncated-toward-zero division, remainder with the
sign of the dividend, overflow and division by zero as controlled errors,
absence of truthiness and implicit conversions -- but its **entry gate lies**:
the lexer does not recognize literals that the language has, and the pending
type table advertises a `Decimal` family that no longer exists.

There is a case worse than a wrong diagnostic: `1.5` is silently tokenized as
`1`, `.`, `5`. The language has no way to say "not yet", because it doesn't
even see the literal.

This is closed **before** Phase 3 because Phase 3 operates precisely on the
misaligned structures -- its task 1.2 manipulates the lexer's phase table --
and because several documented features have no assigned phase: without
assigning them, they either slip into Phase 3 and overflow it, or remain as
undated debt.

## What Changes

### Lexicon: recognize the whole language, implement only what was already implemented

The lexer already has the correct discipline -- recognizing constructs from
later phases so it can say "arrives in Phase N" instead of "unexpected
token". This time it is applied to what was missing:

- `Float` literals (`1.5`, `6.02e23`, width suffixes), hexadecimal and binary
  integers (`0xff`, `0b1010`).
- `**` and `**=`, which today are read as two consecutive multiplications.
- `Char` literals (`'a'`, `'👨‍👩‍👧‍👦'`) and regex literals `re'...'`, with the
  unterminated-regex diagnostic pointing at its opening.
- Interpolation `"value={value}"` with balanced braces, including the one for
  range bounds `0..{number}`.
- Bitwise and shift operators (`&`, `|`, `^`, `~`, `<<`, `>>`) and their
  compound forms, present in levels 7-10 of the precedence table.
- Duration literals (`250ms`, `-3s`, `2h`), without treating `m` as month.
- The keywords that were missing: `do`, `yield`, `interface`, `trait`,
  `strict`, `value`.

None of these gain semantics in this change. All of them are recognized and
deferred with their phase, which is exactly what the lexer already does with
`class` or `task`.

### Pending type table: stop teaching a language that does not exist

- **BREAKING (documentation only):** `Decimal16`, `Decimal32`, `Decimal64`,
  `Decimal128`, `Dec`, and `Decimal` are removed. They are not Zirk types. An
  exact `Decimal` might arrive someday as a library type, and that would be a
  different thing.
- `Float16`, `Float32`, `Float64`, `Float128`, and `Float` are added, the
  integer widths that were missing, and the complete temporal family, each
  declaring its real phase.
- The `Int`, `Integer`, and `UInt` aliases are enabled **now**. `Int` and
  `Integer` are exactly `Int32`, which is already implemented: blocking them
  was blocking a name, not a capability. `UInt` remains deferred together with
  its target `UInt32`.
- Incorrect phase attributions are fixed: `default` belongs to `try`/`catch`
  (Phase 4), not to decorators; `|>` does not belong to Phase 3.

### `String` identity and equality

The observable rule is fixed and optimized underneath:

- `is` compares identity: two bindings that alias the same `String` are
  identical, two with the same content are not.
- `==` compares content and is indifferent to Unicode normalization:
  `"Å" == "Å"` is `true` even if one is in NFC and the other in NFD.
- The hash is computed over the canonical form, so that `Map<String, _>` never
  contradicts `==`.

The opaque handle from ADR-005 is not replaced: **it is** the identity that
`is` compares, and it is what allows graphemes, normalization, and the cache
to live entirely inside the runtime.

### Statement forms that the standard has and the compiler does not

The phase audit uncovered a second group: syntax defined by the standard that
the parser never implemented, and a language rule that was not written down
in any source.

- **The parentheses in a control structure's header are optional.** This was
  a real language rule that no document captured, and that is why the parser
  ended up **requiring** them in the traditional `for`: today
  `for (mut i = 0; ...)` compiles and `for mut i = 0; ...` -- the form the
  standard writes -- is rejected. The rule is written into
  `ZIRK_LANGUAGE_SPEC.md` §5 and both forms are accepted in every structure,
  with the parenthesis-free form as canonical.
- **`do { ... } while condition;`**, the post-condition loop that always
  executes its body once.
- **The effect `if` that governs a statement without braces**:
  `if closed return;`.
- **The ternary `condition ? a : b`**, which the standard prefers for a short
  value choice.
- **`++` and `--` as expressions.** Phase 2 deferred them with a reason
  written in the code: "their prefix/postfix distinction would need an
  evaluation order that no normative document defines". The refined standard
  defines it -- §4 requires preserving the conventional prefix and postfix
  semantics -- so the reason for the deferral has expired. Also, `i++` is the
  canonical step of the documented `for`.

**No program that compiles today stops doing so.** All of these forms
*expand* the set of valid programs: `for (…)` remains valid, it simply stops
being the only option.

### A phase for every documented feature

**Phase 3b -- scalars, conversions, and complete text** is introduced into the
roadmap (integer widths, the `Float` family, graphemic `Char`, deep
contextual conversion, bitwise and shifts, interpolation). They go together
because they depend on each other and all need the Phase 3 contracts.

`inmut::strict` is assigned to Phase 4, where the analysis it needs already
lives. The temporal family and the collections family are assigned to Phase
7, annotated as native types known by the compiler and not as library
objects. Slicing, `Range.step()`/`.reverse()`, and regex also go to Phase 7,
which is where what they need arrives. The `fn gen`/`yield` generators and
the `|>` operator debut in **Phase 7b -- functional style**, numbered this
way so as not to shift phases 8 through 12, which have existed since the
roadmap was written.

### Explicitly out of scope

- **Implementing** `Float` arithmetic, `Char`, the temporal family,
  `inmut::strict`, interpolation, or bitwise operators. This change makes the
  compiler stop lying about them, it does not build them. The statement
  forms from the previous section are the deliberate exception: they are
  pure grammar, with no type work, and the parser already has to be opened
  for the optional parentheses.
- **Phase 3.** Classes, contracts, generics, and data types remain its own
  domain and this change does not touch its scope.
- **Rewriting `String`'s representation.** Its observable behavior is fixed
  and the ADR is amended; the grapheme index is built when the phase that
  indexes it requires it.

## Capabilities

### New Capabilities

- `zirk-feature-phasing`: the verifiable discipline that every documented
  feature has an owning phase, and that one not yet implemented produces a
  diagnostic that names it instead of a syntax error or silence.

### Modified Capabilities

- `zirk-grammar`: optional parentheses in control structure headers,
  `do ... while`, the brace-free `if`, the ternary, and `++`/`--` as
  expressions.
- `zirk-lexical-syntax`: the language's complete vocabulary of literals and
  operators -- Float, hexadecimal, binary, `Char`, regex, duration,
  interpolation, power, bitwise, and shifts -- and the keywords that were
  missing.
- `zirk-type-system`: removal of the `Decimal` family, addition of the
  `Float` family and of the temporal types as pending with a phase, short
  aliases enabled, and `for ... in` over `String` binding `Char`.
- `zirk-runtime-io`: observable identity of the handle, content equality
  indifferent to normalization, and canonical hash consistent with equality.

## Impact

- `crates/zirk-lexer`: `token.rs` (keywords, tokens, phase attribution) and
  `lib.rs` (recognizers for numbers, characters, regex, and interpolation).
- `crates/zirk-parser`: defer the new lexical forms with their phase, and
  accept the statement forms the standard defines -- optional parentheses,
  `do ... while`, brace-free `if`, ternary, and increment as expression.
- `docs/ZIRK_LANGUAGE_SPEC.md` §5: the optional-parentheses rule, which was
  not written down in any source.
- `crates/zirk-sema`: `types.rs` (`pending_type`, aliases, name resolution)
  and the `for ... in` element over `String`.
- `crates/zirk-runtime`: `string.rs`, equality and hash.
- `docs/init/ZIRK_ROADMAP.md`: Phase 3b and the assignment of orphaned
  features.
- `docs/decisions/`: amendment of ADR-005 and a new decision about `String`
  identity, equality, and normalization.
- `crates/zirk-ir` and `crates/zirk-codegen-llvm`: lowering of
  `do ... while`, the ternary, and the increment as expression. This is the
  only part of the change that reaches the binary.
- No program valid today changes behavior: the set of valid programs only
  grows.
