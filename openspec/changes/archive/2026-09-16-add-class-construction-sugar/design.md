## Context

Classes today require explicit fields plus `this.x = x` assignments, and a
class without `construct` cannot be instantiated at all (`E0426`
"`X` has no constructor" is raised at the call site). Construction order is
already fixed: `super(...)` precedes field initializers, which precede the
constructor body. Omitting `super()` in an explicit subclass constructor is
legal today and simply skips the base constructor — base fields then hold
their type defaults. Abstract classes cannot declare constructors, records
have their own named construction, and `inner class` constructors carry a
hidden `outer` parameter.

## Goals / Non-Goals

**Goals:**

- Promoted constructor parameters equivalent to declaring the field plus a
  `this.x = x` assignment, with the full member-modifier surface
  (visibility × mutability), optionals, and defaults.
- An implicit `construct() {}` for classes that declare no constructor,
  with automatic `super()` where the base allows it.
- Diagnostics that read as deliberate design (duplicate field, ambiguous
  multi-constructor promotion, unsatisfiable implicit construction) plus
  the two legibility warnings.

**Non-Goals:**

- Changing what an explicit constructor does: a hand-written
  `construct() {}` in a subclass keeps today's behavior (no automatic
  `super()`). Only the synthesized constructor inserts it.
- Promotion in records, enums, interfaces, traits, or abstract classes —
  abstract classes have no constructors, and records already have named
  construction.
- Generalizing `super()` injection to user-written constructors.
- Property declarations or accessor synthesis of any kind.

## Decisions

### 1. The AST records promotion; sema registers the field

The parser accepts member modifiers only in `construct` parameter position
and flags the parameter as promoted — it does not rewrite the tree into an
explicit field declaration. Keeping the surface form in the AST preserves
parameter spans for diagnostics and lets the formatter and LSP show the
code as written. `zirk-sema` then registers a field entry (marked
"promoted", span at the parameter) in the class member table, so all
downstream machinery — visibility checks, `this.` resolution, layout,
dispatch — sees an ordinary field.

*Alternative considered:* parser-level desugaring into a synthesized field
declaration plus body assignments. Rejected because it loses the written
form too early: diagnostics would point at generated nodes, and the
formatter could not round-trip the sugar without extra metadata anyway.

### 2. Generated assignments run at the top of the constructor body

Lowering emits the `this.x = x` stores as the first statements of the
constructor body — after the `super(...)` call when one is written (a
`super()` call always precedes the body) and after field initializers
(which always run before the body). This matches the documented order and
matches TypeScript, where parameter properties are assigned before body
statements.

*Alternative:* assigning before field initializers. Rejected — it would
make the sugar observably different from the equivalent hand-written
constructor, whose assignments live in the body.

### 3. Promotion requires exactly one `construct`

With multiple `construct` members it is ambiguous which constructor owns
the promoted fields (another overload might not even declare the same
parameters). Rather than merging or picking one, promotion combined with
more than one `construct` is a compile-time error whose cause tells the
user to keep a single constructor or declare the attributes explicitly.
Multiple constructors without promotion keep working unchanged.

### 4. The implicit constructor is synthesized at checking, not parsed in

`zirk-sema` installs a `construct() {}` entry — marked implicit — when a
class has no `construct` member. Because it is not an AST node, it has no
source span, stays out of symbol lists and outlines, and the formatter
never sees it. Instantiation and lowering treat it like a written empty
constructor, so `inmut` fields keep their type default exactly as they do
for a hand-written empty body.

### 5. Implicit `super()` with a warning, unsatisfiable case errors at the call site

For `class B extends A {}`, the synthesized constructor performs an
implicit `super()` when `A` has an accessible no-argument constructor, and
emits a legibility warning recommending an explicit constructor. When `A`
only declares constructors that require arguments, the class declaration
stays legal — consistent with today, where a constructor-less class is
declarable and the failure surfaces at `B()` — and the instantiation
diagnostic names the unsatisfiable base construction.

*Trade-off accepted deliberately:* the implicit constructor auto-calls
`super()` while a hand-written `construct() {}` still skips the base
constructor. The implicit form takes the best-practice behavior and warns;
widening auto-`super()` to explicit empty constructors is a separate
semantic change left for a future proposal.

### 6. Promoted fields warn when used outside their constructor

Reading or writing a promoted field from a method other than its declaring
constructor produces a warning (not an error) suggesting an explicit
attribute declaration for legibility. The warning points at the promoted
parameter so the user sees where the field "really" lives. Uses inside the
declaring constructor — including the implicit `this.` reads — never warn.

### 7. Diagnostics get stable codes and migration-shaped helps

New codes follow the existing `E04xx`/`W0xxx` allocation: duplicate field
(collision), multi-constructor promotion, promotion outside a class
constructor, promoted variadic, unsatisfiable implicit construction;
warnings for promoted-field use elsewhere and implicit `super()`. Every
error names both spellings (short and long form) so the fix is obvious.

## Risks / Trade-offs

- **The legibility warning may fire often.** Any real class that promotes a
  field and then uses it in a method warns — that is the intent (the sugar
  targets constructor-bag classes), but the wording must read as guidance,
  not disapproval. → Keep the warning severity `warning`, never promote it
  in `-D warnings` builds of user code semantics, and document it in the
  handbook.
- **`this.` capture subtleties in closures/nested classes.** A promoted
  field behaves exactly like a declared one, so capture and `inner class`
  rules apply unchanged; tests cover the escaping-capture path.
- **Hidden `outer` parameter of `inner class`.** It is compiler-synthesized
  and never written, so it cannot be promoted; promotion applies only to
  parameters the user writes.
- **Silent `Foo()` on a typo'd class.** A user who forgets to write the
  constructor now gets a default-initialized instance instead of an error —
  accepted, this is the requested feature; the handbook documents that all
  fields receive their defaults.

## Open Questions

- Exact diagnostic codes and message wording (assigned during
  implementation, following existing numbering).
- Whether the promoted-field legibility warning also covers uses inside
  static methods (a promoted instance field is unreachable there anyway,
  so it naturally never fires — confirm while implementing).
