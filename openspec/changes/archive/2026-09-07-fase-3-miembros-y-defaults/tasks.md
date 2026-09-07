# Tasks: fase-3-miembros-y-defaults

## 1. `static` keyword and parsing

- [x] 1.1 Add `static` to the lexer as a keyword (or contextual in member position if it collides with corpus identifiers — grep the corpus first).
- [x] 1.2 Parse `static fn` and `static` fields in class bodies; extend AST member nodes with `is_static`.

## 2. Static semantics

- [x] 2.1 Resolve `ClassName.member` access to static members; keep instance access `expr.member` rejecting statics (and vice versa).
- [x] 2.2 Reject `this` inside `static` bodies; exclude statics from vtables, contract tables, record equality and clone graphs; keep visibility/mutability rules.
- [x] 2.3 Redeclaring a base `static fn` in a subclass creates no override.

## 3. Static lowering and codegen

- [x] 3.1 Lower `static` fields to module globals and `static fn` to free functions (mangled per class); wire `ClassName.member` access to them through IR and LLVM emission.

## 4. Field default initializers

- [x] 4.1 Parse `field: Type = expr;` and store the initializer in the AST.
- [x] 4.2 Sema: initializer must not reference `this`; type-check it against the field type.
- [x] 4.3 Lowering: evaluate defaults in declaration order during construction, before the constructor body, only for fields the constructor does not assign; `super()` runs first for inherited fields.

## 5. Derivation model

- [x] 5.1 Reconcile `zirk-contracts` derivation requirement with the automatic model (delta already written); remove any code/diagnostic references to a `derive` mechanism that does not exist.

## 6. Fixtures and status

- [x] 6.1 Valid fixtures: static method call, static field, field default honored, constructor override of default.
- [x] 6.2 Invalid fixtures: `this` in static body, `this` in field initializer, static access via instance.
- [x] 6.3 Update `docs/init/ZIRK_FEATURE_STATUS.md`.
- [x] 6.4 `cargo test --workspace` green; `openspec validate fase-3-miembros-y-defaults --strict` clean.
- [x] 6.5 Note `./scripts/sync-website-content.sh` as pending post-merge (do not run it).
