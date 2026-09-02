> **Order matters here, unlike in Phase 3.**
>
> Decisions come before code, because the new tokens declare phases the
> roadmap does not yet define. And the lexicon comes before the checker,
> because a pending type with no token producing it cannot be tested.
>
> Each checked box means its rule is covered end to end -- recognition,
> deferral with phase, and tests -- not that a single layer is finished.

## 1. Decisions and roadmap

- [x] 1.1 Amend `ADR-005-representacion-string.md`: the opaque handle is the observable identity that `is` compares, and equality, hashing, and normalization are the runtime's responsibility (D7)
- [x] 1.2 Write the decision on `String` identity, equality, and normalization: `is` by referent, `==` by canonical content, hash over the canonical form, and D5's fast-path order
- [x] 1.3 Add Phase 3b -- scalars, conversions, and complete text -- to `ZIRK_ROADMAP.md`: integer widths, the `Float` family, graphemic `Char`, deep contextual conversion, bitwise and shifts, interpolation
- [x] 1.4 Assign `inmut::strict` to Phase 4 and the temporal family to Phase 7 in the roadmap, annotated as native types known by the compiler and not as library objects
- [x] 1.5 Verify that no feature from the normative sources is left without an owning phase
- [x] 1.6 Assign slicing, `Range.step()`/`.reverse()`, and regex to Phase 7, and create Phase 7b for `fn gen`/`yield` generators and the `|>` operator
- [x] 1.7 Write the rule for optional parentheses in control headers into `ZIRK_LANGUAGE_SPEC.md` section 5, absent from every normative source (D10)

## 1b. Grammar -- statement forms the standard defines

- [x] 1b.1 Accept optional parentheses in the traditional `for` header, which currently requires them, producing the same tree with and without them
- [x] 1b.2 Accept optional parentheses in `for ... in` and in `match`
- [x] 1b.3 Parse `do { ... } while condition;` as a post-condition loop
- [x] 1b.4 Parse the effect `if` that governs a statement without braces, rejecting `else` on that form (D11)
- [x] 1b.5 Parse the right-associative ternary expression `cond ? a : b`
- [x] 1b.6 Admit `++` and `--` in expression position with postfix and prefix semantics, requiring an assignable and mutable place, and remove the diagnostic whose reason had expired (D12)
- [x] 1b.7 Lower `do ... while` to the same basic blocks as `while` with the initial jump inverted, and the ternary to those of the `if` expression
- [x] 1b.8 Lower increment as expression while preserving each form's evaluation order
- [x] 1b.9 Tests: each new form with one valid and one invalid case; `for` with and without parentheses produces the same tree; `i++` as an expression returns the previous value and `++i` the incremented one

## 2. Lexicon -- tokens and keywords

- [x] 2.1 Add the power tokens `**` and `**=`, with longest-match precedence over multiplication
- [x] 2.2 Add the bitwise and shift tokens `&`, `|`, `^`, `~`, `<<`, `>>` and their compound forms, without breaking `&&` or `||`
- [x] 2.3 Add the keywords `do`, `yield`, `interface`, and `trait`. `strict` and `value` remain **contextual**, not reserved: `mut value = 1` and `match r { Ok(value) => ... }` are valid and common code, and reserving them would break it. `inmut::strict` and `value class` are recognized by position
- [x] 2.4 Fix the phase attribution of `default` to the `try`/`catch` phase, of `|>` to the functional-style phase, and of `zirk check`, which promised an already-complete Phase 2 without it
- [x] 2.5 Declare the phase of each new token and keyword according to the now-updated roadmap
- [x] 2.6 Tests: each new token is produced as such, `&&` versus `&` and `**` versus `*` `*`, and each new keyword is not lexed as an identifier

## 3. Lexicon -- numeric literals

- [x] 3.1 Recognize hexadecimal `0x` and binary `0b` integers, with `_` between digits
- [x] 3.2 Recognize fractional and scientific-notation literals as Float literals distinct from integer-dot-integer
- [x] 3.3 Recognize the width suffix on Float literals, preserving it for semantics
- [x] 3.4 Recognize duration literals with the suffixes `ns`, `us`, `ms`, `s`, `m`, `h`, `d`, `w`, admitting a negative sign and without treating `m` as month
- [x] 3.5 Tests: `1.5` produces one literal and not three tokens; `0xff`, `0b1010`, `6.02e23`, `1e2`, `1.5f32`, `250ms`, and `-3s` are recognized; a made-up suffix is still diagnosed

## 4. Lexicon -- text

- [x] 4.1 Recognize character literals between single quotes, preserving their Unicode content whole without validating the grapheme
- [x] 4.2 Diagnose an unterminated character literal pointing at its opening
- [x] 4.3 Recognize regex literals `re'pattern'` preserving escapes for the regular expression parser
- [x] 4.4 Diagnose an unterminated regex pointing at the `re'` opening
- [x] 4.5 Recognize `{ expression }` interpolation in string literals with balanced braces, producing literal parts and expressions as distinct elements (D3)
- [x] 4.6 Apply the same balancing to the interpolated range bound `0..{number}`
- [x] 4.7 Diagnose unterminated interpolation pointing at its opening
- [x] 4.8 Tests: a family emoji produces a character literal with all its code points; nested braces close where they should; each unterminated form points at its opening

## 5. Phase deferral

- [x] 5.1 Defer the Float literal in the checker with its phase diagnostic, without reinterpreting it (D2)
- [x] 5.2 Defer the character literal, the regex literal, the duration literal, and the interpolated string with the phase corresponding to each
- [x] 5.3 Defer the power, bitwise, and shift operators with their phase
- [x] 5.4 Defer the new keywords with their phase
- [x] 5.5 Tests: each new form produces a diagnostic that names it and states its phase, and none produces a generic syntax error or "unrecognized character"

## 6. Type table and aliases

- [x] 6.1 Remove `Decimal16`, `Decimal32`, `Decimal64`, `Decimal128`, `Dec`, and `Decimal` from the pending type table
- [x] 6.2 Add `Float16`, `Float32`, `Float64`, `Float128`, and `Float` with the phase of the Float family
- [x] 6.3 Add the temporal family `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, and `Period` with its phase
- [x] 6.4 Review the declared phase of each remaining pending type against the updated roadmap
- [x] 6.5 Resolve `Int` and `Integer` to `Int32` in the name mapping, so that after resolution they are indistinguishable (D4)
- [x] 6.6 Keep `UInt` deferred with `UInt32`'s phase
- [x] 6.7 Tests: `Decimal64` is an unknown type with no phase; `Float64` and `Instant` declare their phase; `mut count: Int = 0` and `mut count: Integer = 0` compile and are indistinguishable from `Int32`; `UInt` declares `UInt32`'s phase

## 7. `String` iteration

- [x] 7.1 Change the `for ... in` element over `String` to `Char` in the rule, deferring it with the phase diagnostic while `Char` does not yet exist
- [x] 7.2 Tests: iterating a `String` defers with `Char`'s phase and does not bind a `String` element

## 8. `String` identity, equality, and hash

- [x] 8.1 Add the internal field `is_ascii` to the handle, private to the runtime. `normalization`, `grapheme_count`, and `hash` are **not** added: at this phase every `String` is canonical and there is no one who builds the alternative form, so they would be constant fields documenting an intention rather than a state. They arrive with the phase that reads text the compiler did not normalize
- [x] 8.2 Implement equality with D5's path order: same handle, identical bytes, both canonical with different bytes, and only then canonical comparison
- [x] 8.3 Emit string literals already in canonical form from the compiler, marked as such (D6)
- [x] 8.4 Derive the hash from the canonical form, so that two strings equal by `==` never produce different hashes
- [x] 8.5 Tests: NFC versus NFD are equal by `==`; different contents are not; an ASCII string resolves by bytes; hashes of equivalent forms match; a decomposed literal in the source arrives canonical at the runtime

## 9. Closeout

- [x] 9.1 Verify that the existing test suite passes unmodified, except for the tests that explicitly pinned down the corrected behavior
- [x] 9.2 Verify that no `.zrk` program that compiles today changes behavior (D9)
- [x] 9.3 Update the main specs with `openspec sync` and review that Phase 3's task 1.2 remains consistent with the resulting phase table
