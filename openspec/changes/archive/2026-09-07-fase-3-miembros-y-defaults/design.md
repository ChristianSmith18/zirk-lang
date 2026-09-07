# Design: fase-3-miembros-y-defaults

## Context

Classes currently support fields, `construct` overloads, `fn` methods,
visibility, and virtual dispatch — but every member is instance-bound. The
specs reference `static` in passing (`zirk-classes` dispatch rule: private/
static methods do not dispatch virtually) without defining its syntax.
Fields get their *type* default when a constructor omits them
(`has_default()`), but `field: T = expr` is not in the grammar.

On derivation: `zirk-contracts` "Explicit capability derivation" says
classes SHALL NOT derive equality/hashing/cloning silently and records MAY
*request* derivation. Reality: `clone` is offered automatically whenever the
type graph allows it (`is_clone_type`), and `record` equality is automatic
structural equality — no `derive` keyword exists. The spec predates the
implemented model.

## Goals / Non-Goals

**Goals:**
- `static fn` and `static` fields on classes, accessed as `ClassName.member`.
- `field: Type = expr;` initializers evaluated at construction when the
  constructor does not assign the field.
- A documented derivation model consistent with the implementation.

**Non-Goals:**
- `static` on records (records are values; their members stay instance-only
  unless trivially compatible — decided below).
- Companion objects, class-level `const` evaluation, lazy statics.
- Changing the clone/equality semantics that already ship.

## Decisions

1. **`static` is a keyword, not contextual.** `in`/`out`/`value`/`strict`
   taught us positional words create drift; `static` is rare enough as an
   identifier that reserving it is clean. If compatibility matters, the
   lexer can treat it contextually only in member position — the first
   implementation attempts a plain keyword and falls back if corpus tests
   break.
   - `static` fields: restricted to immutable-by-default semantics? No —
     keep the same `mut`/`inmut` model as instance fields; a `static mut`
     field is a global with the usual mutability rules. Note: Zirk's
     runtime is single-threaded until Phase 5, so no synchronization is
     needed; document that Phase 5 revisits this.

2. **Statics live outside the object.** A `static` field lowers to a module
   global; a `static fn` lowers to a free function with a mangled name —
   excluded from vtables, itables, record equality, and clone graphs.
   `this` is an error inside static bodies. Static members obey normal
   visibility.

3. **Field defaults run in declaration order, before constructor body, and
   only for fields the constructor does not assign.** A field default may
   not read `this` (the object is still being initialized) — it is a pure
   initializer expression; if the corpus/spec later wants `this`-dependent
   defaults, that becomes a separate change. If a field has a default, the
   constructor may omit assigning it; without a default, today's
   `has_default()` type-default rules still apply.

4. **Derivation: document the automatic model.** The implemented behavior —
   automatic structural `==` for records, automatic `clone` availability
   when every component is cloneable, `_equals`/operators explicit on
   classes — is consistent and tested. The `zirk-contracts` requirement is
   updated to describe it (classes still do not silently gain equality or
   hashing). No `derive` keyword is added; if explicit derivation is ever
   wanted, it is additive later.
   - Alternative: add `derive`. Rejected — it would churn every existing
     record/clone fixture for no semantic gain.

## Risks / Trade-offs

- [`static` fields are global mutable state] → Allowed but documented;
  Phase-5 thread-safety audit already tracks runtime globals.
- [Field defaults interacting with inheritance order] → Base field defaults
  run via `super()` first; subclass defaults run before its constructor
  body, matching the existing initialization order.
- [`static` keyword breaking identifiers] → Corpus grep for `static` as an
  identifier before choosing plain keyword vs contextual.

## Open Questions

- Whether `static` members are allowed on `record`/`enum` (default: yes for
  `static fn`, no for `static` fields on records — revisit if trivial).
