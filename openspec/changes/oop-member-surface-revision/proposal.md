# Proposal: OOP member surface revision

## Why

The current class-member surface carries ceremony that no longer earns its
place: every method repeats `fn`, `override` is demanded even when there is no
inherited body to replace, and the language has no way to seal a class or a
method against further inheritance. At the same time, classes cannot be
nested, which forces flat name spaces for helper types. This change revises
the member surface so that modifiers exist only where they carry meaning, and
adds the nested-type forms the object model currently lacks.

## What Changes

- **BREAKING** — `fn` is removed from method declarations inside `class`,
  `abstract class`, `interface`, `trait`, and `record` bodies. A method is
  `name(params): Return { ... }`; a field keeps `name: Type`. `fn` remains
  only for top-level functions, function types, `fn dec`, `unsafe fn`, and
  `extern "C" fn`.
- **BREAKING** — `override` becomes a dedicated marker `#override` written on
  its own line above a method. It is required only when replacing an
  inherited implementation (a concrete base method or a trait default).
  Writing it for a signature-only requirement (`abstract class` / `interface`)
  produces a warning; writing it when nothing is overridden is an error.
  The `override` keyword form is removed.
- `final` is introduced as a modifier for classes and methods
  (**normative reversal**: the current spec states "`final` does not
  exist"). `final class` cannot be extended; a `final` method cannot be
  overridden. `final` is not available on attributes — `mut` / `inmut` /
  `inmut::strict` already cover attribute mutability.
- Nested types are added: `class` members may contain nested classes.
  Nesting is static (namespace-only) by default; `inner class` captures a
  reference to the enclosing instance. Classes may also be declared locally
  inside function/method bodies. Anonymous classes are **excluded** —
  closures and local classes cover the idiomatic use.
- **BREAKING** — `mut` is removed from method declarations. Mutability of
  attributes and bindings is unchanged. For `inmut::strict` receivers, the
  compiler now *infers* whether a method mutates `this` (writing `this.*`
  directly or calling another mutating method, computed by fixed-point over
  the call graph) instead of trusting a declared `mut` marker. Calling an
  inferred-mutating method on an `inmut::strict` reference is an error whose
  diagnostic shows the mutation chain. Plain `inmut` is unaffected.
- Lexer gains two surface items: the `#` token (member markers) and the
  `final` keyword.

## Capabilities

### New Capabilities

- `zirk-nested-classes`: nested classes (static by default), `inner` classes
  with an enclosing-instance reference, and local classes in bodies;
  scoping, name resolution, layout, and construction rules.

### Modified Capabilities

- `zirk-classes`: member declaration syntax without `fn`; `#override`
  replacing `override fn` with narrowed obligation; `final` on classes and
  methods (reversing the "final SHALL NOT exist" requirement); `mut` removed
  from methods; static `inmut::strict` call rules updated for inferred
  mutation.
- `zirk-grammar`: member grammar without `fn`; `#override` marker
  production; `final` modifier positions; nested and local class grammar.
- `zirk-lexical-syntax`: new `#` token and `final` keyword.
- `zirk-memory-safety`: the `inmut::strict` method-call rule moves from
  declared `mut` to compiler-inferred mutation; diagnostic requirements for
  the mutation chain.
- `zirk-decorators`: clarification that `#` markers are built-in member
  markers, not decorator applications (`@`), so the two surfaces do not
  collide.

## Impact

- **Parser**: `parse_class_member`/`parse_method` in
  `crates/zirk-parser/src/parser.rs` rewritten for `fn`-less methods,
  `#`-markers, `final`, and nested class members; local class statements.
- **AST**: `MethodDecl` loses `is_override`/`is_mut` (moved to marker
  representation and inferred flag), gains `is_final`; `ClassMember` gains a
  `Class` variant; `ClassDecl` gains `is_final` and enclosing-type context.
- **Semantic analysis**: override checking narrowed to "inherited
  implementation exists"; missing/unnecessary `#override` diagnostics; `final`
  checks on `extends` and override; nested/inner/local class scoping and
  resolution; method-mutation inference over the call graph feeding the
  `inmut::strict` check (`checker.rs`, today at the `is_mut` site).
- **IR / codegen**: nested-type name mangling, `inner` hidden `outer` field
  and constructor plumbing, local-class instantiation.
- **Diagnostics**: new codes for `#override` misuse, `final` violations,
  nested-class errors, and the strict-mutation chain.
- **Docs**: `docs/ZIRK_LANGUAGE_SPEC.md` §7/§12, handbook chapters on
  classes/interfaces/traits, and every `.zrk` example in the repo use the
  old `fn`-method and `override fn` surface and must be updated.
- **Companion repository**: public language documentation and feature-status
  wording change; `../zirk-lang-site` must be updated in the same workstream
  via `./scripts/sync-website-content.sh` after the relevant commits.
- **Migration**: every existing `.zrk` source file (tests, demos, examples,
  handbook listings) must drop `fn` from methods and convert `override fn` →
  `#override` where a body is actually replaced.
