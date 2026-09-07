# Proposal: fase-3-miembros-y-defaults

## Why

Two pieces of the Phase-3 class surface are missing while the spec already
assumes them: `static` members (the classes spec references "private/static
methods" in its dispatch rule but no `static` keyword exists) and explicit
field default initializers (attributes only receive their *type* default;
`x: Int32 = 5` does not parse). In addition, the derivation model is drifted:
`zirk-contracts` describes explicit `derive` requests, while the compiler
actually derives `clone` and `record` equality automatically. That
contradiction must be resolved one way or the other.

## What Changes

- `static` members: `static fn` methods and `static` fields on classes,
  invoked as `ClassName.member`, never dispatched virtually (per the
  existing dispatch rule).
- Field default initializers: `field: Type = expr;` in class bodies; the
  expression is evaluated per construction when the constructor does not
  assign the field.
- Derivation model decision: reconcile `zirk-contracts` "Explicit
  capability derivation" with the implemented behavior — either keep and
  document automatic derivation of `clone`/structural `==`, or add an
  explicit `derive` mechanism. The default direction is to document the
  automatic model (it is implemented, tested, and less invasive), with a
  normative update instead of new syntax.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `zirk-classes`: add requirements for `static` members and field default
  initializers.
- `zirk-contracts`: reconcile "Explicit capability derivation" with the
  automatic derivation model (whichever the decision lands on).
- `zirk-lexical-syntax`: `static` becomes a keyword (or contextual keyword)
  — record the lexical decision.

## Impact

- `crates/zirk-lexer`/`zirk-parser`: `static` keyword, field `= expr`.
- `crates/zirk-ast`: `is_static` on members, field default expressions.
- `crates/zirk-sema`: static access via `ClassName.member`, no `this` in
  static bodies, exclusion from vtables and from record equality,
  initialization order for field defaults vs constructor assignments.
- `crates/zirk-ir`/`crates/zirk-codegen-llvm`: static fields become
  globals; static methods lower as free functions with mangled names.
- Corpus fixtures valid/invalid.
- `docs/init/ZIRK_FEATURE_STATUS.md` updated.
- Normative semantics change (new `static` keyword, field defaults,
  derivation model) → `../zirk-lang-site` sync required after merge.
