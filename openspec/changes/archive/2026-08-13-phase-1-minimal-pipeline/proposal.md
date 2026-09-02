## Why

Phase 0 left the foundations verified — nine crates, diagnostics, emission for the nine targets, and an executing native binary — but **Zirk still doesn't compile a single line of Zirk**. The lexer, parser, sema, and ir crates are empty by design.

This phase connects the entry point. By the end, this file compiles to a real native binary:

```zirk
fn main(): Void {
    stdout.println("Hello from Zirk");
}
```

The value is not the "hello world": it is **validating that the entire backbone works end to end**. Everything that follows in the roadmap extends this backbone rather than building a new one, so a structural mistake here is paid for over the remaining ten phases.

That's why the criterion is a **thin but complete** pipeline, not exhaustive coverage. A parser that covers the whole grammar with no backend connected is worth less, today, than a minimal subset that reaches the binary.

## What Changes

### The language subset

Exactly what `ZIRK_ROADMAP.md` Phase 1 sets is implemented:

- **Declarations**: `fn` with typed parameters and return type; `main` as the entrypoint.
- **Types**: `Void`, `Int32`, `Boolean`, `String`.
- **Variables**: `mut` and `inmut`, with explicit type annotation and inference when unambiguous.
- **Literals**: integers — with `_` as a separator —, strings, `true` and `false`.
- **Operators**: arithmetic `+ - * / %`, comparison `== != < <= > >=`, logical `&& || !`.
- **Control flow**: `if` / `else` as a statement.
- **Calls** to functions defined in the same file, and `return`.
- **Comments** `//` and `/* */`.
- **`stdout.println`** as an intrinsic recognized by the compiler, not as real stdlib.

### The pipeline

- **`zirk-lexer`**: text → tokens with location, including lexical errors with a diagnostic.
- **`zirk-ast`**: the subset's nodes and their spans.
- **`zirk-parser`**: tokens → AST, with minimal error recovery.
- **`zirk-sema`**: name resolution, type checking, and enough flow analysis for the subset.
- **`zirk-ir`**: minimal typed IR, **without assuming a memory model** (ADR-003).
- **`zirk-codegen-llvm`**: IR → LLVM IR → object, extending what already exists.
- **`zirk-runtime`**: `zirk_io_println` over the C ABI boundary, and opaque `String` (ADR-002, ADR-005).
- **`zirk-cli`**: `zirk run` and `zirk build` over a single file, invoking the linker.

### Rules that apply from now on

- **Every language rule with at least one valid case and one invalid case** in tests, per `ZIRK_SPEC_FINAL.md` section 8.
- **Every error with a stable code, cause, and help**, using `zirk-diagnostics`.
- **No numeric truthiness**: `if 1` is a type error, not truth.
- **Ordinary overflow produces a controlled error**, not silent wraparound (`ZIRK_LANGUAGE_SPEC.md` section 3).

### Explicitly out of scope

Generics, classes, `Result`, concurrency, decorators, multi-file modules, `init.zrk`, `match`, loops, closures, nullability (`T?`, `?.`, `??`), `Decimal`, integer types other than `Int32`, and the stdlib beyond `println`.

Also out of scope: **incremental compilation**. The pipeline is single-file and single-pass.

## Capabilities

### New Capabilities

- `zirk-lexical-syntax`: the subset's lexicon — tokens, literals, comments, locations, and lexical errors.
- `zirk-grammar`: the subset's grammar and the construction of the syntax tree, with its errors.
- `zirk-type-system`: subset types, name resolution, mutability, inference, and flow analysis.
- `zirk-ir-lowering`: the minimal typed IR and its generation from the verified tree.
- `zirk-native-codegen`: the translation from IR to LLVM and the production of the linked executable.
- `zirk-runtime-io`: the runtime's C ABI contract for standard output and `String` representation.
- `zirk-cli-commands`: `zirk run` and `zirk build` over a single file.

### Modified Capabilities

- `compiler-workspace`: the crates stop being empty. The *Absence of Zirk syntax at this phase* requirement no longer applies and is replaced.
- `target-matrix`: verification is added that the produced executable runs on the host, not just that the object is emitted.

## Impact

**Crates going from empty to implemented:** `zirk-lexer`, `zirk-ast`, `zirk-parser`, `zirk-sema`, `zirk-ir`.

**Extended crates:** `zirk-codegen-llvm` (lowering from the compiler's own IR), `zirk-runtime` (`zirk_io_println`, `String`), `zirk-cli` (real subcommands).

**New:** a corpus of test `.zrk` files, valid and invalid, with diagnostic snapshots.

**No new external dependencies.** The LLVM pin and the toolchain do not change.

**Risks:**

- **The IR design is this phase's highest-leverage decision.** It is what will be distributed in `.zpkg` (Phase 8) and what an eventual second backend will consume. An IR that assumes a memory model contradicts ADR-003 and would be costly to undo.
- **`String` is the first real test of ADR-005.** If the layout leaks into codegen, Phase 7's grapheme-based indexing becomes a cross-cutting refactor.
- **The scope is tempting to grow.** Loops, `match`, and nullability will look "almost free" once the parser works. They are not: each one drags along type and flow rules, and they belong to Phase 2.
</content>
