## Context

Phase 1 left an end-to-end pipeline but a language with nothing to *structure* a program: no loops, no functions with real shape, no way to say "this may have no value", and no way to split code across more than one file. This phase fills that surface, deliberately without touching objects (Phase 3) or concurrency (Phase 5).

Unlike Phase 1, no IR is designed from scratch here: the one from ADR-007 is extended. The criterion is the same as then -- what cannot be added later without a rewrite is decided carefully now; everything else is postponed.

Current ADRs that constrain this phase:

| Decision | ADR |
|---|---|
| The IR does not assume a memory model | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| IR shape: three-address form, basic blocks, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| `String` is opaque past the runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |

## Goals / Non-Goals

**Goals:**

- Loops (`for`, `for ... in`, `while`, `loop`) with `break`/`continue`, and `if` usable as an expression.
- Functions with optional, named, variadic parameters and default values; closures with immutable capture.
- Exhaustive `match` as an expression and as a statement, over a bounded subset of constructors.
- `T?`, `?.`, `??` working end to end, including their interaction with type checking and codegen.
- `share`/`import`/`use` resolving names across files of the same crate.
- Every new rule with a valid and an invalid test case, as in Phase 1.

**Non-Goals:**

- Classes, inheritance, interfaces, traits, generics, records, unions, enums with associated data (Phase 3).
- `Result`, error handling, real memory strategy (Phase 4).
- Shared mutable capture in closures: it requires the Phase 5 concurrency analysis, which does not exist yet.
- `init.zrk`, external packages, multiple crates (Phase 6, Phase 8).
- Pattern destructuring, `match with` over `Resource<E>`.
- Cross-compilation, compiler performance, LSP.

## Decisions

### D1 -- Exhaustive `match` arrives with a minimal enum, not with Phase 3's algebraic enums

`ZIRK_ROADMAP.md` puts `match` with exhaustiveness "over simple enums" in Phase 2, but "algebraic enums" in Phase 3 together with records, value classes, and unions -- the roadmap's own wording seems to ask for something its own roadmap does not yet allow building.

The resolution: this phase introduces a reduced form of `enum` -- a closed set of constructors **without associated data** (`enum Direction { North, South, East, West }`), sufficient for `match` to have something real to verify for exhaustiveness. Phase 3 extends that same declaration with associated data, generics, and its integration with the rest of the object system; it does not replace it.

**Discarded alternative:** implementing `match` only over literals and the `_` wildcard, with no `enum` type at all. Discarded because the roadmap explicitly asks for exhaustiveness "over enums", and a `match` that is never truly exhaustive does not exercise the feature's central rule.

Patterns admitted this phase: literal, minimal enum constructor, binding variable, `_`. No destructuring, no patterns over unions (they do not exist yet).

### D2 -- Closures capture by immutable copy through an opaque environment, allocated the same way as `String`

```
   closure literal  ──▶  IR: alloc <env>, capture by value  ──▶  opaque pointer to function + environment
```

Same as Phase 1's D3 with `String`: the IR does not say "the environment lives on the heap" or "lives on the stack". It emits an abstract allocation operation (already existing, ADR-003) for the capture environment, and the runtime decides.

Capture is **by value, immutable**: when the closure is created, the captured values are copied into its environment. This is consistent with `ZIRK_LANGUAGE_SPEC.md` section 6 ("a closure safely captures immutable values") and avoids this phase having to resolve aliasing between a closure and its originating scope, which is exactly what Phase 5 (concurrency) does not yet have the means to analyze.

**Discarded alternative:** capture by reference with the environment pointing at the original stack frame. Discarded outright: if the closure escapes the function that created it -- the use case that makes it useful -- the stack frame no longer exists. It would require exactly the escape analysis that Phase 4/5 do not build yet.

### D3 -- `for ... in` iterates over a minimal closed protocol, not over iteration traits

`ZIRK_LANGUAGE_SPEC.md` does not yet define an `Iterable`/`Iterator` trait -- that depends on traits, which are Phase 3. However `for ... in` is requested this phase.

This is resolved by scoping `for ... in` to the types the compiler knows intrinsically this phase: ranges (`0..N`, syntactic sugar recognized by the parser) and `String` iterated by character. There is no user-extensible protocol yet -- a `for x in mi_tipo` over an unrecognized type fails with a diagnostic stating that iteration over user-defined types arrives with Phase 3's traits.

This is the same strategy Phase 1's D4 used for `println`: a bounded intrinsic with a documented retirement date, preferred over building half a trait system ahead of time.

### D4 -- Optional, named, and variadic parameters are resolved in `zirk-sema`, not in the ABI

The signature remains fixed positions in the IR and in codegen: `zirk-sema` reorders named arguments to position, substitutes missing ones with their default value (evaluated at the call site), and packs the variadic arguments into a sequence value before lowering to IR. The IR and LLVM never see a "named argument" or a "missing argument" -- they see a normal call, of fixed arity, already resolved.

**Discarded alternative:** a real variadic ABI (`printf`-style). Discarded because the spec does not ask for interop with variadic C, and a variadic ABI complicates codegen for every call for a benefit no requirement demands yet.

### D5 -- `T?` is a type, not a cosmetic annotation; `??` lowers to an explicit null check in the IR

`T | Null` is represented in `zirk-sema` as its own type, distinct from `T`, which participates in assignment and argument checking like any other. `zirk-ir` lowers `a ?? b` to an explicit null check with two branches -- just like an `if` -- with `b` as the alternative branch. There is no magic coalescing instruction: it is sugar that lowering expands, consistent with Phase 1's D6 (nothing is invented below the IR level that the spec does not ask for).

`?.` would use the same mechanism, but it is deferred for the reason in D8.

### D6 -- Modules resolve names across files of the same crate in a resolution pass prior to type checking

`share` marks a declaration as visible outside its file; without `share`, a declaration is only visible within the file that defines it. `import { X } from "./ruta"` brings that name into the importing file's scope. There is no three-level `public`/`private`/`protected` yet -- that is Phase 3, on classes -- so this phase's visibility is binary: shared or private to the file.

This is resolved as a new pass before the existing type checking: given the set of files in a crate, an `import` graph is built, cycles are detected (an error, with the cycle shown), and it is annotated which `import` declaration resolves to which. Phase 1's type checker does not change shape: only its scope table can now contain entries resolved from another file.

**Discarded alternative:** lazy symbol-by-symbol resolution during type checking. Discarded because it does not allow detecting import cycles cleanly nor before type checking is already halfway through with cascading errors.

### D7 -- `if` as an expression requires type compatibility between branches; without it, it remains a statement

`if`/`else` with both branches present and of compatible type is an expression that produces a value, per `ZIRK_LANGUAGE_SPEC.md` section 5. `if` without `else`, or with branches of incompatible types, remains valid but only as a statement: using it where a value is expected is a type error with a diagnostic explaining which of the two conditions is missing (missing branch or differing types).
### D8 -- `?.` is deferred to Phase 3: there are no members to access yet

Discovered during implementation, not during planning, and it is worth recording rather than forcing through.

`ZIRK_ROADMAP.md` puts `T?`, `?.`, and `??` together in Phase 2, as if they were a single feature. They are not: `T?` and `??` operate on the whole value and work without anything else, but **`?.` is member access**, and in Phase 2 no type with members exists -- classes, records, and traits are all Phase 3. A `usuario?.nombre` has nothing to name.

So `T?`, `null`, and `??` are implemented end to end, and `?.` emits the phase diagnostic already used by the rest of the not-yet-implemented constructs, pointing at Phase 3. The lexer has recognized the token since Phase 1, so the diagnostic is precise and not an "unexpected token".

**Discarded alternative:** inventing a bounded member access just so `?.` has something to do. This contradicts the project's rule against inventing unspecified behavior, and would move up to Phase 2 a decision -- what a member is, how it is resolved -- that belongs to Phase 3's object design.

The nullability that is delivered this phase is not cosmetic: `T?` participates in assignment, argument, and return checking, and `??` lowers to a real null check. What is missing is the navigation operator, not the type system that supports it.

### D9 -- Compound and increment operators enter this phase

`+=`, `-=`, `*=`, `/=`, `%=`, `++`, and `--` do not appear in the roadmap's Phase 2 list, but Phase 1's lexer already recognized them and already declared them as pending for **Phase 2** -- that is, Phase 1 placed them here and the roadmap's list simply does not enumerate them.

This phase also needs them: the three-clause `for` from `ZIRK_LANGUAGE_SPEC.md` section 5 is written `for (mut i = 0; i < 10; i++)`. Leaving `++` out would force writing `i = i + 1` in the language's most common construct.

They are implemented as syntactic sugar that the parser expands to the equivalent assignment: `i += 1` produces the same tree as `i = i + 1`, and `i++` likewise. They do not reach the IR as their own instructions.

**Deliberate limitation:** the prefix/postfix distinction from `ZIRK_LANGUAGE_SPEC.md` section 4 -- that `x++` evaluates to the previous value and `++x` to the new one -- is only observable when the increment is used *as an expression*. This phase admits them only as a **statement**, where both forms are equivalent, and rejects their use in expression position with an explicit diagnostic. Admitting them as an expression requires fixing the evaluation order of side effects within an expression, which no normative document defines yet.


### D10 -- A closure cannot escape in this phase, so its captures travel inside the value itself

Discovered while implementing D2, and it changes its cost entirely.

D2 decided to capture by value through an environment allocated with ADR-003's abstract operation. Implementing it surfaced the problem: the runtime only knows how to allocate `String`, and a general allocation would be the language's first -- exactly the decision ADR-003 fixes for Phase 4.

But that allocation is not needed, because **in this phase a closure cannot escape the function that creates it**. There is no syntax to write a function type in an annotation: `(Int32) => Int32` does not exist as a type, so a closure can only live in an inferred local. It cannot be returned, passed as a parameter, or stored in a field -- fields are Phase 3.

So the closure value carries **its captures within itself**, not a pointer to an environment:

```
   closure  =  { function pointer, capture₁, capture₂, … }
```

and the lambda body is hoisted to a module function whose first parameters are the captures. Calling it means extracting the pointer, extracting the captures, and calling with `(captures…, arguments…)`.

Consequences:

- **Zero allocation.** The value lives wherever its slot lives, on the stack frame, and ADR-003 is untouched.
- **Every lambda has its own type**, identified by the lambda and not by its signature. This is correct because there is no way to write a function type: at every use site the type is statically known.

  The first version of this decision added that "it goes unnoticed", and that turned out false the second time it was tested. **Reassignment notices it**: `mut F = (): Int32 => 1; F = (): Int32 => 2;` left the slot with a value whose layout no longer matched. This is rejected with its own diagnostic, because saying that `(Int32) => Int32` is not `(Int32) => Int32` is useless to anyone.

  And **capturing a closure inside another also notices it**: the checker identifies a function type by its own numbering and the IR by the layout it built, and these are not the same. The type of a capture is taken from the slot that holds it, not from the checker's type -- the slot is the only thing that knows which layout is involved.
- **Capture remains by value and immutable**, exactly as D2 fixed it. Nothing about D2 is undone: only where the environment lives changes.

When Phase 3 gives function types syntax, a closure will be able to escape, and at that point it will indeed be necessary to decide where its environment lives. That decision arrives together with the memory decision, which is where it belongs, and not ahead of time.

## Risks / Trade-offs

- **D1's minimal enum could remain as debt if Phase 3 does not extend it carefully** → Mitigation: it is declared without associated data right from the requirement's name ("simple enum"), and the roadmap's Phase 3 requirement ("algebraic enums") is explicitly understood as an extension of this same declaration, documented here so Phase 3 inherits it instead of discovering it.

- **D2's capture by value may surprise someone expecting capture by reference** → Mitigation: this is literally what `ZIRK_LANGUAGE_SPEC.md` section 6 asks for; the diagnostic on an attempt to mutate a captured variable must explain that the capture is immutable, not just reject the mutation.

- **D3 scopes `for ... in` to ranges and `String` -- this may tempt extending it type by type before traits exist** → Mitigation: same mechanism as Phase 1's D6, an explicit diagnostic with the phase where it is resolved, instead of adding types to the intrinsic ad hoc.

- **D6 introduces a new pass before type checking, which touches the `zirk-cli` driver** → Accepted trade-off: it is the only clean way to detect import cycles, and the driver already orchestrates stages (Phase 1, `compile()`), so adding one does not change its shape.

## Migration Plan

Additive over a pipeline that already works. Every new construct that today fails with Phase 1's D6 diagnostic ("this exists but is not implemented yet") starts compiling.

Rollback: revert the merge. Phase 1 does not depend on anything from this phase.

## Decisions made during implementation

Four that were not planned, each discovered while writing the code rather than while planning:

- **`?.` is deferred to Phase 3** (D8). The roadmap groups it with `T?` and `??` as if they were one feature, and they are not: `?.` accesses a member, and no type in this phase has members.

- **Compound and increment operators enter here** (D9). Phase 1 already marked them as pending for Phase 2, and the language spec's three-clause `for` is written with `i++`.

- **A closure's captures travel inside the value** (D10). A closure cannot escape in this phase, so there is no need to allocate an environment -- and not allocating is what avoids moving up ADR-003's memory decision.

- **A `Span` names its file**, in [ADR-010](../../../docs/decisions/ADR-010-ubicaciones-multiarchivo.md). Modules break the single-file assumption, and the cheap alternative -- rustc-style global offsets -- gets in the way of Phase 9's LSP. This went to an ADR and not here because it is a contract with Phase 11's debugger and with Phase 8's `.zpkg`: a change gets archived, an ADR does not.

## Findings that only showed up in CI

Two bugs the four platforms did not share, and that no amount of testing on a single machine would have found:

- **A lambda's name has to be a valid symbol.** Hoisted functions were named `<lambda>#0`, and `#` is the comment character in AT&T-syntax assembly. It broke only on x86_64; aarch64 comments with `//`. They are now named `lambda.N`, and a test pins down the admitted character set.

- **PIC has to be emitted outside Windows.** Linux distributions link as PIE, and a PIE does not admit the absolute relocations `RelocMode::Default` produces on x86_64. It is triggered by taking a function's address -- exactly what building a closure does -- so no Phase 1 program noticed it.

Both are consequences of the same feature, and both showed up on only one of the matrix's four platforms. This is the concrete justification for why ADR-004 requires verifying on all of them.

## Corrections to this same document

- **The requirement to reject `import` cycles had no reason behind it.** `import` brings names into scope and nothing in this phase depends on the order in which files are read. Two files that reference each other mutually are a normal program; what must be avoided is walking the cycle indefinitely, and that is resolved by reading each file once. The requirement was rewritten.

- **A variadic collects its values into a sequence, and there is no collection type until Phase 3.** It is accepted grammatically and reported with E0423 instead of compiling incorrectly.

## Open Questions

- **What happens if two files of the same crate declare the same shared name?** `ZIRK_LANGUAGE_SPEC.md` section 10 does not say.

  **Resolved: it is a collision error, with no tie-break by file order.** A crate has a single namespace in this phase, so the collision is detected even if the declarations are private. The diagnostic names the other file, because a lone line number says nothing when they are in different files. Per-module namespacing belongs to Phase 6's project system.

- **Does `for x in 0..N` include `N`?** The range sugar is not defined in the normative documents read for this phase.

  **Resolved: `0..N` exclusive, `0..=N` inclusive.** This is the most widespread convention and avoids the most common use case -- "N times" -- having to write `N - 1`. Both forms have an end-to-end test that pins down the behavior.
