# Design — inferred-return-types

## Context

The parser requires `: T` on every `fn`, method, contract member and `extern`
declaration via the shared `parse_return_type` helper
(`crates/zirk-parser/src/parser.rs:1655`), which emits
`MISSING_RETURN_TYPE` when the colon is absent. In the checker
(`crates/zirk-sema/src/checker.rs`), signatures are collected in a
declaration pass (`declare_function` / `declare_members`) **before** any body
is checked, so a function may call another declared later in the file or in a
mutually importing module. `check_function` then validates each body against
`current_return`, and `check_return` already implements the
declared-type contract, including the D14 capturing-closure fix-up. The
entrypoint check (`check_entrypoint`) requires `main`'s resolved signature to
be `(): Void`. `Type::unify` (`crates/zirk-sema/src/types.rs:556`) already
provides the branch-join semantics this inference reuses (`Never` contributes
nothing, `Null` contributes nullability, different bases fail).

## Goals / Non-Goals

**Goals:**

- Omitting `: T` on `fn` and on methods with bodies resolves the return type
  from the body's `return` expressions, enforced afterwards exactly like a
  declared type.
- Inference completes before any caller needs the signature — including
  callers in mutually importing files, since `frontend.rs` merges all units
  into one `Program` before checking.
- `zirk check --types` emits a span→type map the VS Code extension turns
  into TS-style hovers, refreshed on save like diagnostics.

**Non-Goals:**

- Parameter-type inference — parameters keep mandatory annotations.
- Lambda return-type inference — lambdas already use contextual inference
  (`CORE_LANGUAGE_SEMANTICS.md` §80).
- Hover for unsaved buffers, go-to-definition, autocomplete — Phase 9 LSP.
- Inferring through `unsafe`/`extern`/`dec`/bodyless signatures — annotation
  stays mandatory there by spec.

## Decisions

### D1 — `Option<TypeRef>` in the AST, not a sentinel type

`FnDecl.return_type` and `MethodDecl.return_type` become
`Option<TypeRef>`. `parse_return_type` splits into
`parse_optional_return_type` (returns `None` when no `:` follows) and a
`require_return_type` wrapper used by `parse_contract_method`,
`parse_extern_fn`, `parse_fn_dec`, and `parse_method` when `is_abstract` —
the four bodyless contexts where inference is impossible and the diagnostic
must stay. Emitting the error from the *parser* for bodyless signatures keeps
the rule "no body → mandatory annotation" a syntax fact, not a sema pass.

*Alternative considered:* keep `TypeRef` and add `TypeRef::Inferred` marker.
Rejected — every existing `resolve_type(&f.return_type)` call site would need
an unwrap anyway; `Option` makes the absence explicit at the type level and
is what the codebase already does for `throws: Option<TypeRef>`.

### D2 — A dedicated inference pass between declaration and body checking

Signature collection stays exactly where it is. Unannotated functions and
methods register their signature with `returns: Type::UNKNOWN` plus a
`pending_inference` flag. Between `declare_*` and `check_function`/
`check_class`, a new pass `infer_return_types(program)` runs:

1. For each unannotated function/method, its body is checked **in collect
   mode**: the same `check_function` machinery runs the whole body — return
   expressions can depend on locals (`mut x = 5; return x;`), so partial
   checking is not an option — but `check_return` records each returned
   type `(span, Type)` into a per-function collection instead of enforcing
   it against `current_return`, which is set to an infer-mode sentinel.
   After the body: `unify` the collected types → resolved return type →
   written into the registered `Signature`, replacing `UNKNOWN`. No
   `return expr` → `Void`; a body that provably always diverges with no
   `return expr` → `Never` (the all-paths analysis `check_block` already
   returns).
2. Each recorded `(span, actual)` is then validated against the resolved
   type — the same mismatch diagnostic as a declared annotation — and the
   usual all-paths rule applies. Because the function's body is now fully
   checked, it is marked so the later `check_function`/`check_class` loop
   does not check it twice.

A consequence of infer-mode: return expressions are checked with **no
expected type**, so context-dependent expressions (collection literals,
numeric literals) take their default type and may then mismatch the
resolved one — the same outcome a declared annotation produces, reported
by the same diagnostic.

**Pipeline placement.** The pass runs once, after *every* declaration pass —
functions, externs, and class members — so an inferred body can call anything
the program declares. That is a deliberate reorder of today's sequence:
`check_conformance`, `require_pending_overrides`, and
`infer_method_mutation` are all *consumers* of resolved signatures, so they
move after `infer_return_types` (and before `check_entrypoint`, which also
consumes `main`'s resolved type). They cannot stay before it: conformance
comparing an unannotated method against a contract would read `UNKNOWN`, and
inferring the method earlier is impossible because its body may call free
functions whose signatures are not declared yet at that point.

**On-demand recursion with a `inference_in_progress` set:** while inferring
`f`, its body may call `g` whose signature is also pending. Call resolution
consults `pending_inference`; finding `g` there triggers `infer_one(g)`
before proceeding — which is itself a collect-mode body check, so per-
function state (`current_return`, `in_main_scope`, `pending_throws`,
`unsafe_depth`) is saved and restored around it exactly as `check_function`
already does for `pending_throws`. A pending function found *inside* the
in-progress set is an inference cycle → diagnostic naming the cycle,
"annotate one member" help, and `UNKNOWN` propagated so checking continues.

*Alternative considered:* lazy inference inside `check_direct_call` with no
dedicated pass. Rejected — `main`'s signature must be resolved for
`check_entrypoint`, which runs before body checking; a dedicated pass
guarantees every signature is concrete before *any* consumer sees it, with
no ordering assumptions.

*Alternative considered:* infer by checking only the `return` expressions,
not the whole body. Rejected — `return x` where `x` is a local requires the
preceding statements to have been checked; a partial check would duplicate
scoping rules the full check already implements.

### D3 — Infer first, then enforce identically to declared

Inference produces a type; then `check_function`/`check_class` run unchanged
and validate every `return` against it — bare `return;` against an inferred
`Int32` is the same `TYPE_MISMATCH` as against a declared `Int32`, and a
falling-off-the-end body gets the same `MISSING_RETURN`. This keeps one
enforcement path; inference only decides what `current_return` starts as.
The unification of valued returns happens in the D2 pass; if `unify` fails,
a dedicated `INCOMPATIBLE_INFERRED_RETURNS` diagnostic names the conflicting
types, and `UNKNOWN` keeps subsequent checking graceful.

### D4 — The checker records a type map during its existing traversal

`zirk check --types` needs span→type entries. Rather than a second AST walk,
the checker records entries as it resolves them: declaration names
(`fn`/`method`/`mut`/`inmut`/param/field) with their resolved signature or
type — flagged `inferred` when the return type came from D2 — and expression
types for hovered constructs (calls, identifiers, literals). Storage: a
`Vec<TypeMapEntry>` on the checker, keyed by `(FileId, Span)`; emitted as
JSON by `zirk check --types` through the existing `RenderStyle::Json`
pipeline — a sibling render, not a new diagnostic kind.

*Alternative considered:* recompute types in the extension or a separate
`--hover line:col` query. Rejected — the extension must never re-implement
checking (editor-tooling spec), and per-query invocation can't cache a whole
file's map like the on-save refresh model allows.

### D5 — Extension consumes the map; no parsing of its own

The extension gains a `HoverProvider` that keeps a per-document cache of the
type map, refreshed inside the same post-save check the bridge already runs
(editor-cli-bridge). Position→entry is a smallest-containing-span lookup;
the hover renders `fn hola(): Void` / `count: Int32` in a Zirk code block,
with `(inferred)` appended when flagged. No hover is produced when the map
is absent or the position matches nothing. Extension bumps to `0.5.0`
(package.json + `.vsix` naming convention already used for 0.1.x–0.2.0).

## Risks / Trade-offs

- **Nested body-checking during inference** → D2's sub-check runs
  `check_expr` on `return` expressions only, inside the function's own
  scope push/pop — it does not run `check_function` recursively, so no
  re-entrancy of per-function state (`in_main_scope`, `pending_throws`)
  is needed. The in-progress set is the only new mutable state.
- **`main` inferred non-`Void`** → `check_entrypoint` reads the resolved
  signature after the D2 pass, so `fn main() { return 0 }` correctly fails
  `INVALID_ENTRYPOINT`; the error message already explains the expected
  signature.
- **Type-map size on large files** → entries are recorded only for spans
  the checker already resolves; `--types` is opt-in, so default `zirk
  check` pays only a `Vec` push per entry, no serialization.
- **Saved-file-only hover** → acceptable: the bridge already refreshes on
  save; hovering mid-edit shows the last saved map, matching how
  diagnostics behave today. Documented as a bridge limitation, not an LSP
  promise.
- **Trait default bodies + conformance** → a trait method inferring `Int32`
  while an implementer annotates `Int64` surfaces the existing
  signature-mismatch diagnostic at `check_conformance` — no new machinery.

## Migration Plan

Purely additive: every existing program keeps its declared types and their
exact enforcement. `MISSING_RETURN_TYPE` remains but narrows to bodyless
contexts; corpus fixtures exercising it (`invalid/` cases for interface,
abstract, extern) are updated, and new `valid/`/`invalid/` fixtures cover
each spec scenario. No user-facing migration.
