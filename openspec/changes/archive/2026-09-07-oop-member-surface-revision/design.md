# Design: OOP member surface revision

## Context

Zirk's class surface today requires `fn` on every method, `override fn` on
every replacement or requirement satisfaction, forbids `final` by normative
decision ("Classes are inheritable by default and `final` does not exist",
`ZIRK_LANGUAGE_SPEC.md` §7), supports `mut fn` to mark receiver-mutating
methods consumed by the `inmut::strict` check in
`zirk-sema/src/checker.rs`, and has no nested, inner, or local classes.

Current pipeline facts that constrain the design:

- `parse_class_member` (`zirk-parser/src/parser.rs` ~line 845) parses
  modifiers then dispatches on `construct` / `fn` / field.
- `MethodDecl` (`zirk-ast/src/lib.rs` ~line 288) carries `is_override`,
  `is_mut`, `is_static`, `is_abstract`.
- The checker stores `is_mut` into `MethodInfo` and rejects `mut` calls on
  `inmut::strict` roots (~line 12137).
- The lexer has no `#` token and no `final` keyword.
- Decorators (`@Name`) are specified but unimplemented; `#` must not
  collide with that future surface.
- Interfaces already accept plain (non-`override`) implementations; only
  `abstract class` requirements currently demand `override fn`
  (`checker.rs` ~line 2707, `MISSING_OVERRIDE`).

## Goals / Non-Goals

**Goals:**

- `fn`-less method declarations inside `class`, `abstract class`,
  `interface`, `trait`, and `record` bodies.
- `#override` marker, required exactly when an inherited *implementation*
  is replaced; warning when unnecessary; error when it overrides nothing.
- `final` on classes and methods; `abstract`+`final` rejected.
- Nested classes (static by default), `inner` classes with an enclosing
  instance reference, and local classes in bodies.
- `mut` removed from methods; receiver-mutation inferred by the compiler
  for the `inmut::strict` check.

**Non-Goals:**

- Anonymous classes (`Contract() { ... }`) — deferred; closures and local
  classes cover the idiomatic use and are documented as the replacement.
- `final` on attributes — `mut`/`inmut`/`inmut::strict` already cover it.
- General `@` decorator implementation — `#override` is a built-in member
  marker, not a `fn dec` application, and does not unblock the decorator
  pipeline.
- Method overloading, multiple inheritance, or any change to `extends` /
  `implements` arity.
- Changing plain `inmut` semantics — only `inmut::strict` interacts with
  method calls.

## Decisions

### D1 — Member grammar without `fn`

Inside any type body, after modifiers, the parser sees an identifier and
disambiguates by lookahead:

```
member      := markers? modifiers* ident memberTail
memberTail  := typeParams? '(' params ')' (':' type)? throws? (block | ';')   // method
             | ':' type ('=' expr)? ';'                                       // field
             | 'construct' ...                                                // constructor
             | 'class' ...                                                    // nested class
```

- `ident (` or `ident <` opens a method; `ident :` opens a field.
- Function-typed fields keep their type after the colon
  (`handler: fn(Int32): Void;`), so `ident (` is unambiguous — field types
  never begin with `(` at that position except parenthesized types, which
  the grammar does not support there; verify against `parse_type` and, if
  needed, forbid a leading `(` type in field position.
- `fn` remains a keyword for top-level functions, function types,
  `fn dec`, `unsafe fn`, and `extern "C" fn`. Writing `fn` inside a type
  body produces a targeted diagnostic ("methods do not use `fn`") rather
  than a generic parse error, to aid migration.

*Alternative considered:* keep `fn` optional. Rejected — two spellings for
the same thing fragments the corpus and every example.

### D2 — `#override` member marker

- Lexer gains `TokenKind::Hash`. A `#`-marker line is parsed as part of
  the member prefix, before modifiers: `#override` then the method header.
- The marker is grammatical, not positional — "on its own line" is a
  style convention; the parser accepts `#override` wherever a member
  prefix is legal. Formatter/linter may enforce the line break later.
- Semantic rule (replaces the current `override fn` rule):

  | Situation | `#override` |
  |---|---|
  | Replaces a concrete base method or a trait default | required — missing is an error naming both declarations |
  | Satisfies an `abstract class` or `interface` requirement (no body existed) | forbidden — warning "`#override` is unnecessary; remove it" |
  | Overrides nothing | error |
  | On a field, constructor, nested class, or `static` member | error (methods only; constructors are not inherited) |

- AST: `MethodDecl.is_override: bool` keeps its meaning ("the marker was
  written") — renaming the field is optional; the parser just sources it
  from `#override` instead of the `override` keyword. `Keyword::Override`
  is removed from member parsing; leaving the keyword as a reserved word
  for one release aids error messages.

*Alternative considered:* `@override` as a decorator. Rejected — the
decorator pipeline is unimplemented and `override` is a compile-time
check, not an expansion.

### D3 — `final` on classes and methods

- New `Keyword::Final`; modifier parsed in class position
  (`final class Foo`) and member position alongside visibility/`static`.
- `final class`: `extends Foo` where `Foo` is final → error. Final classes
  still may `implements` contracts and be instantiated.
- `final` method: any `#override` (or unmarked redeclaration, which is
  already an error) naming it in a subclass → error.
- `final` + `abstract` on the same class → error (contradictory).
- `final` method inside a `final class` → warning (redundant).
- `final` on fields, constructors, interfaces, traits, records → targeted
  error; records are value types and already non-inheritable.
- This **reverts** the normative line "Classes are inheritable by default
  and `final` does not exist" in `ZIRK_LANGUAGE_SPEC.md` §7 and the
  `zirk-classes` spec requirement "Single inheritance".

*Alternative considered:* sealed-by-default. Rejected — the language keeps
inheritable-by-default and adds opt-in sealing.

### D4 — Nested, inner, and local classes

```
class Outer {
    class Nested {}        // static nested (default): type `Outer.Nested`
    inner class Inner {}   // captures the enclosing instance
}

fn f(): Void {
    class Local {}         // visible only inside the block
}
```

- **Static nested** (default): pure namespacing. The nested class is a
  normal class whose canonical name is `Outer.Nested`; it has no access to
  `Outer`'s instance and no implicit reference. It may itself contain
  nested classes (arbitrary depth). Visibility modifiers apply to the
  nested class as a member.
- **`inner class`**: the layout gains a hidden first field
  `outer: Outer` and its constructor gains a hidden leading parameter.
  Inner methods may reference `outer` and may access `private`/`protected`
  members of the enclosing class (and vice versa: the outer class may
  access private members of its inner classes — same decision as Java).
  Construction requires an enclosing instance: inside `Outer`, `Inner()`
  binds `this`; outside, `o.Inner()`. An `inner` class may not declare
  `static` members (no enclosing-independent state).
- **Local classes**: `class` becomes a valid statement inside any body.
  Scope is the enclosing block; the class may capture nothing (v1: local
  classes do **not** capture local variables — that is what closures and
  explicit fields are for; revisit only if a concrete need appears). The
  symbol gets a mangled canonical name (`f$Local`) and is invisible
  outside its function.
- Name resolution: nested classes enter the class's member namespace and
  the module's type namespace under their qualified name. Cycles through
  `extends`/`implements` involving nested types reuse the existing
  inheritance-cycle check.
- AST: `ClassMember::Class(ClassDecl)`; `ClassDecl` gains
  `is_inner: bool` and `enclosing: Option<...>` context supplied by the
  parser/sema. Local classes reuse `Decl::Class` at statement level.
- IR/codegen: nested/inner/local classes lower like ordinary classes with
  mangled names; `inner` adds the hidden `outer` field to the object
  layout (before user fields, after the header) and the hidden parameter
  to every constructor.

*Alternatives considered:* (a) all nested classes capture (Java non-static
inner as default) — rejected: wastes a field per object and surprises;
Kotlin-style explicit `inner` is cheaper and clearer. (b) anonymous
classes — deferred per the proposal.

### D5 — `mut` removed from methods; mutation inferred for `inmut::strict`

- The `mut` keyword is no longer accepted before a method. `MethodDecl`
  drops `is_mut`; `MethodInfo` replaces it with an inferred
  `mutates_receiver: bool`.
- **Inference**: for each class, compute the set of mutating methods by
  fixed-point over the intra-class call graph:
  - a method *mutates* if its body assigns to `this.field` (including
    through a local place rooted at `this`, e.g. `this.a.b = x`), or
    calls `this.m()` where `m` mutates;
  - iterate until a fixed point; recursive cycles are handled by
    computing SCCs of the call graph (or equivalently iterating to
    convergence — the lattice is monotone).
- **Strict rule**: `inmut::strict` receiver + call to an
  inferred-mutating method → error. The diagnostic MUST show the mutation
  chain (`c.reset() → helper() → this.count = 0`) so the user can see why
  an undeclared method counts as mutating.
- **External boundary**: `extern "C"` functions and any callable without
  an analyzable body are conservatively assumed mutating; `inmut::strict`
  cannot cross that boundary.
- Plain `inmut` receivers: no method-call restriction (unchanged).
- `mut` on a method becomes a parse error with migration guidance
  ("`mut` on methods no longer exists; mutation is inferred").
- `unsafe {}` internals: a write to `this.*` inside `unsafe {}` still
  counts as mutation for inference purposes.

*Alternative considered:* reject every method call on `inmut::strict`.
Rejected — it makes strict references useless on classes (even getters
would fail). Inference preserves the intent ("freeze the reachable graph")
without a user-visible marker.

*Risk carried into E:* inference is **non-local** — making a private
helper start mutating can break `inmut::strict` call sites in other
modules. Accepted trade-off; mitigated by chain-reporting diagnostics.

### D6 — Ordering inside the member parser

Member prefix order (all optional, at most once each):

```
#-markers*  visibility?  static?  abstract?  final?  mut?  inner?
```

- `mut` before a non-`fn` member is now *field* mutability only; `mut`
  followed by `(`-shaped member tail is the removal diagnostic of D5.
- `inner` is only legal on a nested `class` member.
- `abstract` remains legal only inside `abstract class` (existing rule).

## Risks / Trade-offs

- [Grammar ambiguity `ident (` vs function-typed fields] → resolved by
  the colon placement of field types (D1); add parser tests for
  `handler: fn(Int32): Void;`.
- [`#` token may collide with a future attribute/directive syntax] →
  documented in `zirk-decorators` delta: `#` is reserved for built-in
  member markers, `@` for `fn dec` decorators.
- [Mutation inference is non-local: editing a private method can break
  distant `inmut::strict` call sites] → diagnostics show the full mutation
  chain; documented in the spec delta and handbook.
- [Every `.zrk` file in the repo breaks (removed `fn`, `override`)] →
  migration sweep in the same change; the parser emits targeted migration
  diagnostics for `fn method` and `override fn`.
- [`inner` adds hidden state and constructor plumbing to layouts] →
  confined to classes explicitly marked `inner`; static nested and local
  classes keep ordinary layouts.
- [Large single change touches parser, sema, IR, codegen, docs] → tasks
  are ordered so the member-syntax revision (A/B/C/E) lands and migrates
  first, then nested types (D) build on the settled grammar.

## Migration Plan

1. Land lexer tokens (`#`, `final`) — additive, no breakage.
2. Land `fn`-less member parsing + `fn`-in-type-body diagnostic, migrate
   all in-repo `.zrk` sources.
3. Land `#override` semantics (required/warning/error matrix), migrate
   sources.
4. Land `final` checks.
5. Land mutation inference and remove `mut` from methods.
6. Land nested → inner → local classes in that order.
7. Update `ZIRK_LANGUAGE_SPEC.md` §7/§12, handbook chapters, feature
   status, then sync `../zirk-lang-site` per repo convention.

Rollback: the change is source-breaking by design; rollback is a revert
of the whole change plus restored sources.

## Open Questions

- Construction syntax for `inner` classes outside the outer body:
  `o.Inner()` is assumed; confirm against the grammar's postfix rules.
- Whether local classes should eventually capture enclosing locals —
  v1 says no.
- Formatter rule for `#override` line placement (deferred to tooling).
