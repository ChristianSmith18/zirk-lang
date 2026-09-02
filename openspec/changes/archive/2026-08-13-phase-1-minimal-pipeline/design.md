## Context

Phase 0 verified the final stretch of the pipeline: given LLVM IR, the project produces a native binary that runs, on Linux, macOS, and Windows. What's missing is everything before that.

This phase is the only one in the roadmap where **all** layers are touched at once. The decisions made here — especially the shape of the IR — constrain the following ten phases, so the criterion is not "whatever makes hello world run" but "what won't have to be undone in Phase 3."

Active ADRs that constrain this phase:

| Decision | ADR |
|---|---|
| The IR does not assume a memory model | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| The runtime is a staticlib with a C ABI boundary | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| `String` is opaque behind the runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| LLVM 20.1 pin | [ADR-001](../../../docs/decisions/ADR-001-pin-llvm.md) |

## Goals / Non-Goals

**Goals:**

- `zirk run hola.zrk` compiles and runs; `zirk build hola.zrk` produces an executable.
- The subset from `ZIRK_ROADMAP.md` Phase 1 works end to end.
- Every language rule has a valid case and an invalid case in tests.
- Every compiler error comes out with a stable code, location, cause, and help.
- The IR is typed and does not assume a memory representation.

**Non-Goals:**

- Compiler performance. Single pass, no cache or incrementality.
- Quality of the generated code. `OptimizationLevel::None` is sufficient.
- Grammar coverage. What is not in the subset must **fail with a clear diagnostic**, not be parsed halfway.
- Cross-compilation. Host only; sysroots are Phase 6.
- Public Syntax API, formatter, linter, LSP.

## Decisions

### D1 — The tree is an AST typed per node, not a generic tree

`ZIRK_COMPILER_SPEC.md` section 3 mentions a "syntax tree" and a versioned public Syntax API. These are two different things: the internal AST is private and can evolve; the Syntax API arrives in Phase 10 together with decorators.

An AST is implemented with one type per construct (`FnDecl`, `IfStmt`, `BinaryExpr`, …), not a homogeneous tree of generic nodes with children.

**Discarded alternative:** a homogeneous rowan/CST-style tree, which would be better for an LSP with error recovery and trivia. Discarded because the LSP is Phase 9, and adopting a CST now imposes complexity across every layer for a benefit that takes eight phases to arrive. When it does arrive, it is introduced as an additional layer under the AST, not in its place.

Every node carries its span, without exception: a node without a location cannot produce the diagnostic the spec requires.

### D2 — The IR is three-address, typed, in basic-block form

```
   Verified AST  ──▶  IR  ──▶  LLVM IR
                       │
             typed, with basic blocks
             and abstract allocation operations
```

A basic-block IR is chosen instead of a tree, even though a tree would suffice for this phase's subset, because:

- `ZIRK_COMPILER_SPEC.md` section 4 requires it to retain enough information for escape analysis, devirtualization, and vectorization — all flow analyses, which are awkward over a tree;
- Phase 2 introduces loops and `break`/`continue`, which over a tree would force reworking the representation;
- the mapping to LLVM is direct, because LLVM is already exactly that.

**SSA with phi functions is not adopted yet.** Local variables are represented as slots with load and store, and promotion to registers is delegated to LLVM. A hand-rolled SSA is real work that doesn't pay off until there are custom optimizations.

### D3 — Allocation is an abstract IR operation

Direct consequence of ADR-003. The IR **does not** say "malloc", "gc_alloc", or "refcount_inc": it expresses `alloc <type>` and the runtime decides.

At this phase the only thing allocated is `String`, and the runtime resolves it however it wants. When Phase 4 chooses the memory strategy, the runtime and the lowering change, not the IR.

### D4 — `println` is an intrinsic, not stdlib

`import { stdout } from std.io;` is not implemented: there are no multi-file modules at this phase.

The compiler recognizes `stdout.println(<expr>)` as a special syntactic form and lowers it to a call to the runtime's `zirk_io_println`. This is deliberate, bounded debt, retired in Phase 7 once real `std.io` exists.

**This is preferred over implementing half a module system**, which is Phase 2 and drags in `share`/`import`, path resolution, and visibility.

The diagnostic for an `import` must explicitly say that modules arrive in a later phase, not "unknown symbol."

### D5 — `String` crosses the boundary as an opaque handle

Application of ADR-005. Codegen emits:

```
  zirk_str_from_utf8(ptr, len) -> ZirkStr     // literal
  zirk_io_println(ZirkStr)                     // output
```

`ZirkStr` is an opaque pointer as far as the compiler is concerned. That the runtime today implements it as `{ptr, len}` is invisible from the IR.

### D6 — Unsupported constructs fail with an explicit phase diagnostic

A `class`, a `for`, or a `match` must not produce "unexpected token." The lexer recognizes the reserved words of the full language, and the parser emits a diagnostic saying the construct exists but is not implemented yet.

This is consistent with the project's rule of not inventing unspecified behavior, and it turns the subset into something comprehensible instead of a different language that happens to resemble Zirk.

### D7 — The linker is invoked from `zirk-cli`, not from codegen

`zirk-codegen-llvm` produces objects; linking is orchestration. At this phase, the `clang` from the pinned LLVM installation is invoked, which is already required, instead of relying on each host's `cc` — the same criterion ADR-004 applied to the linker.

## Risks / Trade-offs

- **The IR is designed with a trivial subset yet must serve classes and generics** → Mitigation: basic blocks and explicit typing from the start, which is what cannot be added later without a rewrite. What is deferred (SSA, optimizations) is additive.

- **`println` as an intrinsic might stick around** → Mitigation: the requirement declares it as debt with a retirement date in Phase 7, and the `import` diagnostic references that phase.

- **The typed AST makes Phase 9's LSP harder** → Trade-off accepted in D1, with the exit path noted: the CST is introduced as an additional layer.

- **The subset is tempting to grow** → Mitigation: D6 makes unsupported constructs fail clearly and cheaply, which removes the pressure of "while we're at it, let's add `for`."

- **Overflow checking at compile time vs. runtime** → The spec requires a controlled error, not wraparound. For constants it is detected at compile time; for runtime operations, LLVM offers intrinsics with detection. The intrinsics route is used even at a performance cost: the alternative contradicts the spec.

## Migration Plan

Not applicable: it is purely additive on top of empty crates.

Rollback: revert the merge. Phase 0 does not depend on anything from this phase.

## Open Questions

All three were resolved during implementation. They are kept along with their resolution instead of being deleted: the question explains why the answer wasn't obvious.

- **Does `zirk run` leave the executable on disk or delete it?** `ZIRK_COMPILER_SPEC.md` section 9 does not specify this.

  **Resolved: it leaves it, in `build/`.** Whoever ran their program most likely wants to distribute it, and a compiler that hides its output forces a second command to retrieve it. There is a test that pins this down.

- **What exit code does `main` produce at this phase?** `ZIRK_RUNTIME_SPEC.md` section 2 delegates this to an "entrypoint contract" that is not defined.

  **Resolved: only `Void`, with exit code 0.** The checker rejects any other `main` signature with a diagnostic stating which one is expected. The underlying question — what it means for `main` to return a `Result` — remains open and belongs to Phase 4, once `Result` exists.

- **Does inference cover `mut x = 5`?** The spec allows it "when unambiguous."

  **Resolved: yes.** With a single integer type in the subset, inference is unambiguous. Forbidding it now and allowing it later would have been an incompatible change in the tests.

## Decisions made during implementation

Two that were not anticipated and were documented where appropriate:

- **The shape of the IR moved to [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md).** A change's `design.md` gets archived; the shape of the IR is a contract with Phase 8 and needed a durable home.

- **Zirk functions carry a `zk_` prefix in generated code.** This prevents a Zirk function called `printf` from colliding with the C one, and leaves the `main` name free for the entrypoint the operating system invokes. The scheme is internal and will be revisited when packages arrive in Phase 8.
</content>
