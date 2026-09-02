## Context

The two previous phases avoided allocating. Phase 1 had a single allocatable thing —`String`— and hid it behind the runtime boundary (ADR-005). Phase 2 came to have closures and resolved them **without allocating anything**: since they cannot escape, their captures travel inside the value itself, on the stack frame (D10).

This phase has no such way out. An object with identity outlives the frame that created it, and that is its whole point. It is the central tension of the design and the first decision below.

Everything else is representation decisions that become contracts: an object's layout and its method table travel inside Phase 8's `.zpkg`, just like the shape of the IR (ADR-007). Choosing them poorly is expensive later.

Standing ADRs that constrain this phase:

| Decision | ADR |
|---|---|
| The memory strategy is decided in Phase 4; the IR does not name it | [ADR-003](../../../docs/decisions/ADR-003-memoria.md) |
| The runtime is a staticlib with a C ABI boundary | [ADR-002](../../../docs/decisions/ADR-002-runtime-staticlib.md) |
| IR shape: three addresses, basic blocks, slots | [ADR-007](../../../docs/decisions/ADR-007-forma-de-la-ir.md) |
| `String` is opaque behind the runtime | [ADR-005](../../../docs/decisions/ADR-005-representacion-string.md) |
| A `Span` names its file | [ADR-010](../../../docs/decisions/ADR-010-ubicaciones-multiarchivo.md) |

## Goals / Non-Goals

**Goals:**

- `class` with `public mut` fields by default, methods, multiple `construct`, `this`, visibility, and single inheritance.
- Interfaces and traits, with traits contributing implementation.
- Generics `<T>` with `from` constraints.
- Records, value classes, algebraic enums, and unions.
- Casts `as` and `<T>`, with controlled failure in the checkable form.
- Retire the three debts Phase 2 deferred to this phase: `?.`, `+` on `String`, and extensible `for ... in`.
- Every new rule with one valid and one invalid test case, as in the two previous phases.

**Non-Goals:**

- Choosing the memory strategy. The boundary is provided; the choice is Phase 4.
- `unsafe {}` and casts that reinterpret memory.
- The collections library, and with it the variadic parameter that awaits it.
- Generators, `|>`, and the rest of the functional style from section 8.
- Variance, associated types, higher-order generics: the spec does not ask for them.
- Function type syntax, and with it closures that escape (D9).
- Decorators, reflection, errors, concurrency.

## Decisions

### D1 — The runtime allocates objects through an abstract operation, and still does not choose a strategy

`ZIRK_LANGUAGE_SPEC.md` requires an object to have identity, and ADR-003 forbids the IR from naming a memory strategy. The two are compatible if the split is the one ADR-002 already fixed: **the IR expresses `alloc <type>` and the runtime materializes it**, with the runtime being the single point where the decision is made.

So this phase adds to the runtime an allocation function behind the C ABI boundary, and **does not decide what it does internally**. What it will do in this phase is the simplest thing that satisfies the contract: reserve and never free.

Not freeing is not an oversight: it is the consequence of ADR-003 placing the choice in Phase 4. Freeing requires having decided *when*, and that is exactly the open question. A program from this phase terminates and the operating system reclaims everything.

**Discarded alternative:** bringing forward reference counting because "it is the easiest thing." ADR-003 already rules it out as a final strategy —`RUNTIME_SPEC` §9 requires freeing cycles, and pure RC cannot— so building it here would be work that later has to be undone, with the added trap that a half-built RC seems to work until the first cycle appears.

It stays noted as debt with a deadline: the requirement declares it and Phase 4 retires it.

### D2 — An object is a header plus its fields, and the header starts out being just the type

> Recorded as a durable decision at the close of the phase: [ADR-012](../../../docs/decisions/ADR-012-layout-de-objetos.md).

```
   object  =  [ type descriptor | field₁ | field₂ | … ]
```

The descriptor identifies the type at runtime. It is what dynamic dispatch, checkable casts, and `ZIRK_LANGUAGE_SPEC.md` section 12 need when it says that "basic type identity always exists."

Inherited fields go **before** the type's own fields, in the order the hierarchy declares them. This way the layout prefix of a subclass matches that of its superclass, and accessing an inherited field is the same offset no matter who looks. This is what makes single inheritance free at the access site.

The header carries **only** the descriptor for now. Whatever Phase 4 needs for memory —marks, counters, whatever the strategy asks for— gets added there, and that is why it exists as a separate concept from the start instead of appearing only when needed.

### D3 — Virtual methods are dispatched through a table in the type descriptor

> Recorded as a durable decision at the close of the phase: [ADR-013](../../../docs/decisions/ADR-013-forma-del-despacho.md).

The descriptor points to a method table, and a call to a virtual method is an indirect load through it. This is the representation that makes single inheritance cheap: the subclass's table starts with the superclass's entries, in the same order, so a method's index does not change on inheritance.

**Interfaces do not fit that scheme** —a class implements several and each would want its own indices— so every implemented interface carries its own table, and the descriptor keeps the list of the ones the class satisfies. A call through an interface looks up its table and dispatches through it.

**Not every method is virtual.** A method that no subclass overrides is called directly: that is most calls, and paying an indirection for all of them would be paying for generality the program does not use. Determining which ones are overridable is information the checker already has, because it knows the full hierarchy of the crate.

**Discarded alternative:** dispatch by name lookup at runtime, in the style of dynamic languages. It is simpler to implement and slower on every call, and `ZIRK_COMPILER_SPEC.md` section 4 requires the IR to keep enough information to devirtualize — which presupposes that there is something to devirtualize.

### D4 — A trait is an interface that can also bring implementation, not a separate mechanism

`ZIRK_LANGUAGE_SPEC.md` section 7 names them separately and distinguishes them by a single thing: "traits can include reusable implementation." There is no second difference in the spec.

They are then implemented as the same mechanism: a contract with methods, where an interface is the case where none of them carry a body. A trait method with a body is copied into the class that adopts it if the class does not override it, so there is nothing new in the table layout.

Being the same mechanism does not make them the same keyword: the spec distinguishes them and the diagnostic must too, because writing `interface` with a body is an error that deserves to be reported as such.

**Conflict between two traits that contribute the same method:** it is an error, and the class resolves it by overriding it. Choosing by declaration order would be a silent tie-break rule, exactly what Phase 1 avoided by not admitting implicit conversions.

### D5 — Generics are checked once and specialized when lowered

Checking happens on the generic itself: `fn f<T from Serializable>(x: T)` is verified **once** against the constraint, not on every instantiation. A use that does not satisfy `from` fails at the call site, with the concrete type in the diagnostic.

The IR **does** receive a copy per combination of types used. This is what `ZIRK_LANGUAGE_SPEC.md` section 7 calls specializing "where appropriate," and what avoids requiring a generic `T` to exist at runtime.

**Discarded alternative:** type erasure with everything going through a pointer. It would make it impossible to store value classes inline, which `RUNTIME_SPEC` §9 requires and ADR-003 records as a non-negotiable constraint.

Left out, because the spec does not ask for them: variance, associated types, higher-order generics, and explicit user-driven specialization.

### D6 — Operators are overloaded through reserved contracts, and that is what turns `+` on `String` into concatenation

`ZIRK_LANGUAGE_SPEC.md` section 4 says an operator can only be overloaded through language contracts and that overloading does not alter precedence or arity. This phase fixes the reserved names (`_add`, `_subtract`, etc.), allows implementing them on user types, and forbids reopening native types. `String` internally implements the concatenation contract, and `"a" + "b"` starts working.

The consequence that matters is one of direction: the checker stops having a fixed list of types per operator and starts looking up the contract instead. Integers and booleans still resolve directly —they are of the language, not of a library— but they do so through the same path.

### D7 — `?.` arrives now because there are now members

Phase 2's D8 deferred `?.` for a concrete reason: it accesses a member and no type had members. That reason disappears with classes.

It comes down the same way as `??`: an explicit nullity check with two blocks, where the present branch accesses the member and the absent one produces `null`. The result's type is the member's type, in its nullable form. The mechanism already existed since Phase 2 (D5), so what arrives is the operator, not the machinery.

### D8 — `for ... in` now requires `Iterable<T>`, and Phase 2's closed protocol is retired

Phase 2's D3 restricted `for ... in` to ranges and `String` because there were no traits, and noted that iteration over user-defined types would arrive with them. They arrived.

`Iterable<T>` and `Iterator<T>` are defined as language contracts, and `for x in e` now requires the type of `e` to implement `Iterable<T>`. Ranges and `String` stop being special cases of the compiler and start implementing it, which is what makes a user-defined type indistinguishable from a language one in a `for`.

### D9 — Phase 3 does not yet deliver the final function-type syntax

This was the open question the design started with, and it is resolved before writing code because it constrains what can be done with a closure.

**Implementation limit of this phase.** The final accepted syntax is
`Function(P...) => R`, with the preferred alias `Fn(P...) => R`, signature
compatibility, and escapable closures with automatic storage. Phase 3 does not
implement it yet: its parser and checker reject the annotation with a
phase-availability diagnostic. The rejection does not constitute final
semantics and does not make the future type per-expression nominal.

Consequently, throughout Phase 3 a closure:

- **can** be stored in a local variable whose type is inferred;
- **can** be invoked within the scope where its concrete type is known;
- **cannot** be annotated as a parameter, return, or field type;
- **cannot yet** escape the function that creates it in the Phase 3 compiler;
- temporarily keeps its captures inline, without prejudging the final
  automatic representation.

**Why not now.** Introducing the syntax would turn closures into values interchangeable by signature: they could be stored in objects, returned, and received as arguments. That forces deciding **where the environment lives and how long it lasts**, which is a memory decision, and memory decisions belong to Phase 4 (ADR-003). It would be bringing forward exactly what D1 of this phase is careful not to bring forward.

**Semantics already decided outside the delivery scope.** Functions, lambdas,
compatible methods, and callable objects adapt to the `Fn` type; closures can
escape; parameters are contravariant and returns covariant; identity uses
`is`; and assignment shares the environment while `clone()` deep-copies it.
The later phase that implements this surface must follow the canonical
checkpoint, not infer rules from Phase 3's temporary limitation.

Using a lambda where a type annotation is required produces a diagnostic stating that function types belong to a later phase, not "unknown type."

### D10 — Later authorial decisions prevail over inherited drafts

The authorial clarifications from August 2026 fix the surface this phase shares with the handbook: there is no ordinary shadowing; `this.name` disambiguates a capture that collides with a lambda parameter; fields are `public mut` by default; there can be several `construct`; named arguments select and reorder parameters; and traditional enums expose the case name unless an explicit `->` mapping exists. These rules are verified in the parser and checker before fixing lowering or layout.

### D11 — Shared references obey the same mutability matrix

`String`, arrays, collections, and classes share the public rule: `mut` allows
reassigning and mutating, `inmut` only prevents reassignment, and
`inmut::strict` freezes the reachable graph. A strict reference does not
produce mutable aliases and cannot be acquired from a still-accessible
mutable alias; `clone()` creates an independent logical copy when the contract
exists. Phase 3 applies this rule to the objects and contracts it
introduces; the concrete analysis and memory strategy still belong to the
corresponding phases. Only moving the entire variable shares the reference.
Reading an attribute, index, slice, destructured component, or pattern
binding produces an independent, deep logical copy; the same path used as a
place does mutate the original storage. A reference projection requires
`Clone`.

## Risks / Trade-offs

- **Allocating without freeing is correct for this phase and a leak as soon as a program runs long** → Mitigation: the requirement declares it as debt with a deadline in Phase 4, and no partial freeing that would later have to be undone is built (D1).

- **Object layout is a contract with Phase 8's `.zpkg`** → Mitigation: what is fixed now is what cannot be changed later without a rewrite —the order of inherited fields and the header's existence—; whatever Phase 4 needs gets added to the header, which exists from the start for that reason (D2).

- **The scope is the largest in the roadmap and everything touches everything else** → Classes need generics to be useful, generics need contracts to constrain, contracts need classes to be implemented. There is no order in which one part finishes before the others, so `tasks.md` advances by pipeline layers rather than by language construct.

- **Specialized generics multiply the generated code** → Accepted trade-off: it is the only way to satisfy ADR-003's constraint of inline value classes, and the cost in size is measured when Phase 8 addresses binary size.

- **Retiring the closed `for ... in` protocol changes code that already works** → Ranges now implement `Iterable<T>`, so existing programs must keep compiling unchanged. Phase 2's corpus is the proof, and it is not touched.

## Migration Plan

Additive on top of a pipeline that works end to end. Constructs that today fail with the phase diagnostic start compiling.

Three things that are rejected today become accepted and require the existing corpus to stay green: `?.`, `+` on `String`, and `for ... in` over user-defined types.

Rollback: revert the merge. No earlier phase depends on this one.

## Open Questions

The question about function types was resolved before starting and became D9.


- **Can a value class have virtual methods?** Section 7 says they have no observable identity and are stored inline. A virtual method needs a runtime type descriptor, and keeping one in something stored inline contradicts "compact."

  **Confirmed at closure (14.3):** no. A record or value class's own method is always dispatched statically — `IrType::Value` resolves through the same `method_of` as an object, but since neither admits `extends`, `method.overridden` is never true and it never goes through `CallVirtual`. `implements` is accepted and its conformance is checked the same way as for a class, but reaching the method through the contract type (the only reason dynamic dispatch would matter for a type with no identity) is blocked with `NOT_LOWERED` on the declaration itself: there is no descriptor where the contract table could be stored. Found and fixed at closure: calling a record's own method directly used to crash (it had never been tested; `Self::method_of`/`lower_method_call` only recognized `IrType::Object`); corpus in `value_class_methods.zrk` (direct call, lowers) and `record_implements_not_lowered.zrk` (through the contract, gated).

- **What happens when comparing two records with `==`?** Section 7 says a record has structural semantics, which suggests derived field-by-field equality. Section 4 says `==` is already structural equality for everyone.

  **Decided, pending lowering (14.3/14.4):** for a record or value class it is derived automatically, with no `_equals`; for a class with identity, `==` compares identity unless the class implements the operator contract — this second half already lowers (D6). The first type-checks (`check_equality` accepts `==`/`!=` on a record without requiring `_equals`) but its lowering —walking field by field, including a record nested inside another— is gated with its own `NOT_LOWERED` (`reject_unstructured_comparison`, roadmap task 11.5): building the inline value (11.5) was one piece; comparing two such values structurally is another that no task in this phase claimed as its own. It remains as live debt with no phase assigned yet — see 14.4.
