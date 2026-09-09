# Compiler Pipeline

Zirk uses one staged frontend for checking, compilation, formatting, linting,
editor services, documentation, and validated decorator expansion. Native
builds continue through portable IR and LLVM; frontend-only tools stop before
those stages.

```text
source files and .zkinit
        ↓
source/module loading
        ↓
lexing and lossless syntax
        ↓
semantic AST
        ↓
module and name resolution
        ↓
type and flow analysis
        ↓
decorator Inspect → Augment → Wrap
        ↓
generated-code resolution, typing and validation
        ↓
typed portable IR
        ↓
LLVM IR → object → linker → native artifact
```

> **Implementation status:** the current compiler has working batch loading,
> lexing, recursive-descent parsing, semantic AST, name/type/flow checking,
> typed IR verification, LLVM emission, linking, and a static runtime for its
> implemented subset. The persistent lossless tree, fine-grained incremental
> frontend, full module namespaces, decorator engine, and many later language
> features remain target architecture. See
> [`COMPILER_IMPROVEMENT_SUGGESTIONS.md`](../../../COMPILER_IMPROVEMENT_SUGGESTIONS.md)
> for the observed integration state and implementation guidance.

## Stage ownership

| Stage | Compiler owner | Produces | Must not do |
| --- | --- | --- | --- |
| Source loading | driver/project loader | canonical source graph and snapshots | resolve language names or grant permissions |
| Lexer | `zirk-lexer` | tokens, trivia/syntax input and byte spans | decide grammar or types |
| Parser | `zirk-parser` | recoverable syntax and semantic AST input | perform type checking |
| Resolution/checking | `zirk-sema` | resolved, typed and flow-validated program | choose object layout or emit code |
| Decorator engine | compiler frontend | generated validated syntax and expansion maps | bypass visibility, types, permissions or hygiene |
| Portable IR | `zirk-ir` | verified target-independent executable form | commit to LLVM or a memory strategy not required by semantics |
| Native backend | `zirk-codegen-llvm` | LLVM module and object | redefine language behavior |
| Driver/linker | `zirk-cli` | requested artifacts and diagnostics | contain compilation semantics |
| Runtime | `zirk-runtime` | stable native services used by generated code | expose private layouts as source semantics |

Dependencies flow in pipeline order. `zirk-diagnostics` is the deliberate
cross-cutting exception: every stage can construct the same stable diagnostic
shape without depending on another stage.

## Loading reachable source

Compilation begins at an entry source or the entry declared by `.zkinit`. The
loader follows imports rather than compiling every `.zrk` in a directory. An
unreachable file is not part of the program.

Each file receives a stable identity in the compilation source map. The target
model preserves both its user-facing written path and canonical module identity,
deduplicates aliases/symlinks, prevents project-root escape, and permits access
to dependencies only through the manifest. Mutual imports do not make the file
loader recurse forever; semantic dependency rules decide whether the resulting
declaration cycle is valid.

The current loader already walks reachable local imports, reads each normalized
path once, accepts mutual file traversal, and continues loading after parse
errors to produce useful project-wide diagnostics. Current compilation still
temporarily merges declarations into one crate namespace; per-module resolution
is later work.

## Frontend-only workflows

`zirk check`, formatter, linter, LSP, documentation, and compatible decorator
inspection share source snapshots and frontend results. They do not need LLVM,
a platform linker, SDK, runtime archive, or native dependencies.

```text
zirk check       frontend through semantic/decorator validation
zirk format      lossless syntax and canonical formatting
zirk lint        syntax + resolved/type information required by each rule
zirk doc         public typed API and documentation
LSP              incremental snapshots and recoverable frontend state
zirk build/run   complete frontend + IR + backend + linker
```

The current CLI does not yet provide the LLVM-independent `check` path. It is a
documented target and a priority integration boundary, not a claim about the
current executable.

## Stage gates

A stage may recover to report more diagnostics, but an error cannot be converted
into evidence that the program is valid:

1. Lexical and parser recovery produce marked tokens/nodes.
2. Semantic recovery may use an internal unknown/error type to prevent cascades.
3. Any frontend error prevents decorator commit or IR lowering as applicable.
4. Generated decorator code passes the same resolution, typing, effect, safety,
   and visibility checks as source code.
5. Portable IR is independently verified before backend emission.
6. An invalid IR is an internal compiler error, never blamed on user source.

## Feature completion states

During development, a feature can be recognized, parsed, checked, lowered, or
supported by codegen/runtime. These states help maintain the compiler but are
not interchangeable:

| State | Meaning |
| --- | --- |
| Recognized | lexer/tooling knows the syntax and can name it accurately |
| Parsed | recoverable AST/syntax representation exists |
| Checked | semantic rules and invalid cases are enforced |
| Lowered | verified portable IR preserves the feature |
| Backend/runtime ready | native execution implements the required behavior |
| Implemented | every stage required by its public use is complete and tested |

Private-development diagnostics retain roadmap phase information through the
standard `cause` and `help` fields. A central feature-stage catalog should keep
code, tests, status pages, and diagnostics synchronized.

## Revalidation and traceability

Every source and generated declaration retains spans. Decorator expansion adds
an origin chain from generated syntax to decorator application and generating
operation. Diagnostics and debugger locations therefore show the meaningful
source plus expansion context instead of an opaque generated file.

Changing a declaration invalidates its dependent names, types, expansions, and
IR. Changing only a function body should not invalidate unrelated public APIs.
The detailed cache and dependency model is covered by
[Incremental Compilation](15-incremental-compilation.md).

## Failure boundaries

Source errors use stable lexical, grammar, resolution, type, effect, safety, and
tool diagnostics. Missing toolchain components use build/toolchain diagnostics.
Compiler invariants use an internal-error category containing version, target,
pipeline stage, and safe reproduction guidance. Zirk never uploads source or
telemetry automatically.

---

**Previous:** [← Toolchain](README.md) · **Next:** [Lexer and Parser →](02-lexer-and-parser.md)
