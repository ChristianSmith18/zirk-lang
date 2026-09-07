# Tasks: OOP member surface revision

## 1. Lexer: new tokens

- [x] 1.1 Add `TokenKind::Hash` (`#`) to `zirk-lexer` and produce it for `#`; verify `#override` lexes as `Hash` + `override` keyword.
- [x] 1.2 Add `Keyword::Final` to the lexer keyword table; add a regression test that `final` is a keyword, not an identifier.
- [x] 1.3 Keep `Keyword::Override` tokenized (now used only after `#`); confirm no lexer change needed for it beyond documentation.

## 2. AST: member model changes

- [x] 2.1 Update `MethodDecl` (`zirk-ast`): keep `is_override` as "the `#override` marker was written", add `is_final: bool`, remove `is_mut` (mutation becomes inferred), keep `is_static`/`is_abstract`.
- [x] 2.2 Add `is_final: bool` to `ClassDecl`; add `is_inner: bool` and enclosing-class context needed for nested/inner classes.
- [x] 2.3 Add `ClassMember::Class(ClassDecl)` for nested classes; add a member-marker list field on `MethodDecl` (or equivalent) so `#override` is represented distinctly from modifiers.
- [x] 2.4 Add a statement-level `class` declaration form for local classes (reuse `Decl::Class` inside bodies).
- [x] 2.5 Update every `Program`/`ClassDecl`/`MethodDecl` construction site, visitor, and test fixture for the new fields.

## 3. Parser: `fn`-less methods and member markers

- [x] 3.1 Rewrite `parse_class_member` to parse `#`-markers, then modifiers (`public|private|protected`, `static`, `abstract`, `final`, `mut`, `inner`), then dispatch: `construct` → constructor; `class` → nested class; `ident` + `(`|`<` → method; `ident` + `:` → field.
- [x] 3.2 Remove the `fn` requirement from `parse_method`; emit a targeted migration diagnostic when `fn` appears inside a type body ("methods no longer use `fn`").
- [x] 3.3 Parse `#override` before a method; store it on the method; error with "markers apply only to methods" when it precedes a field/constructor/nested class; error on unknown `#name`.
- [x] 3.4 Parse `final` in class position and member position; emit targeted errors for `final` on fields, constructors, `interface`, `trait`, `record`, parameters, and variables ("attributes use `inmut`, not `final`").
- [x] 3.5 Parse `inner class` only as a class member; error elsewhere.
- [x] 3.6 Accept `class` declarations inside function/method/constructor bodies (local classes).
- [x] 3.7 Emit a targeted diagnostic for anonymous-class syntax `Type() { ... }` recommending a local class or closure.
- [x] 3.8 Ensure function-typed fields (`handler: fn(Int32): Void;`) still parse as fields, not methods; add parser tests for the disambiguation.
- [x] 3.9 Emit a removal diagnostic for `mut` before a method ("`mut` no longer applies to methods; mutation is inferred").

## 4. Semantic analysis: `#override`, `final`, and inferred mutation

- [x] 4.1 Replace the `override`-required rule: `#override` is required only when replacing an inherited implementation (concrete base method or trait default); missing marker → error naming both declarations.
- [x] 4.2 Remove the `MISSING_OVERRIDE` requirement for `abstract class`/`interface` satisfaction; a written `#override` there produces a warning ("unnecessary, remove it").
- [x] 4.3 Error when `#override` replaces nothing; error on `#override` on static methods/fields/constructors/nested classes.
- [x] 4.4 Implement `final` checks: `extends` of a `final class` → error; `#override` of a `final` method → error; `abstract final class` → error; `final` method inside `final class` → warning.
- [x] 4.5 Implement method-mutation inference: compute per-class mutating methods by fixed-point over the intra-class `this`-call graph (write to `this.*` or call to a mutating method); handle recursive cycles via SCC/fixpoint.
- [x] 4.6 Rewire the `inmut::strict` method-call check (`checker.rs` ~line 12137) to use inferred mutation; diagnostics must show the mutation chain (`reset → helper → this.count = ...`).
- [x] 4.7 Treat methods without analyzable bodies (`extern "C"`, external symbols) as mutating for the strict check.
- [x] 4.8 Update trait-conflict resolution to require `#override` (replacing `override fn`) while keeping `TraitName.super.method()` unchanged.

## 5. Semantic analysis: nested, inner, and local classes

- [x] 5.1 Register nested classes under qualified names (`Outer.Nested`) in the class table; member visibility applies to the nested class name.
- [x] 5.2 Nested static classes: no enclosing-instance access; unqualified enclosing-field references inside them error.
- [x] 5.3 `inner` classes: resolve `outer` to the enclosing instance; allow mutual private/protected access between inner and enclosing class; reject `static` members inside `inner class`; require an enclosing instance at construction (`o.Inner()` outside, `Inner()` inside binds `this`).
- [x] 5.4 Local classes: block scoping, mangled canonical names, invisible outside the function; reject capture of enclosing locals with a targeted diagnostic.
- [x] 5.5 Extend inheritance-cycle and contract-cycle checks to nested/inner/local classes.

## 6. IR and codegen

- [x] 6.1 Lower nested/inner/local classes with mangled canonical names; ensure vtables, layouts, and descriptors use the qualified identity.
- [x] 6.2 `inner` class layout: hidden `outer` field after the object header, before user fields; hidden leading constructor parameter; wire `o.Inner()` construction.
- [x] 6.3 Verify virtual dispatch, `is`/`as`/`as?`, and `clone()` work on nested/inner/local class instances.
- [x] 6.4 Update the IR verifier for the new layout shapes (hidden `outer` field).

## 7. Diagnostics and migration of in-repo sources

- [x] 7.1 Add/adjust diagnostic codes: `#override` misuse, `final` violations, nested/inner/local class errors, `fn`-in-type-body migration, `mut`-on-method removal, anonymous-class rejection.
- [x] 7.2 Migrate every `.zrk` file in the repo (root demos, `test_*.zrk`, fixtures): remove `fn` from methods, convert `override fn` → `#override` where a body is replaced, drop `mut` from methods.
- [x] 7.3 Migrate all parser/sema/codegen test sources to the new surface.

## 8. Documentation and companion site

- [x] 8.1 Update `docs/ZIRK_LANGUAGE_SPEC.md` §7: `fn`-less methods, `#override` rule, `final` (removing "final does not exist"), `mut` removal, nested/inner/local classes; §12 note that `#` markers are not decorators.
- [x] 8.2 Update `COMPILER_IMPROVEMENT_SUGGESTIONS.md` where it references the old member surface.
- [x] 8.3 Update handbook chapters covering classes, interfaces, traits, records, and `inmut::strict`; update feature-status and current-limitations pages.
- [ ] 8.4 After the relevant commits, run `./scripts/sync-website-content.sh` and update `../zirk-lang-site` status wording per repo convention (with `--audit-date` if project-status evidence changed).

## 9. Verification

- [x] 9.1 Parser tests: method without `fn`, `fn`-in-body diagnostic, `#override` positions, `final` positions, nested/inner/local class trees, anonymous-class diagnostic, field/method disambiguation.
- [x] 9.2 Sema tests: `#override` required/warning/error matrix, `final` matrix, mutation-inference cases including transitive and recursive calls, strict-receiver accept/reject with chain diagnostics.
- [x] 9.3 End-to-end: `oop_demo.zrk` (migrated) compiles and runs identically; a new demo covering nested/inner/local classes.
- [x] 9.4 Full workspace `cargo test` passes; LLVM-dependent tests run with `LLVM_SYS_201_PREFIX=/opt/homebrew/opt/llvm@20`.
