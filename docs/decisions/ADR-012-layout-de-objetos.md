# ADR-012 — Object layout

- **Status:** accepted
- **Date:** August 19, 2026
- **Phase:** 3

## Context

`ZIRK_LANGUAGE_SPEC.md` section 12 requires that "basic type identity always exists" at runtime, and Phase 3 is the first one that has something with identity: an object outlives the frame that created it, unlike everything Phases 1 and 2 allocated (D1 of `fase-3-objects-and-type-system/design.md`).

An object's layout is also a contract with Phase 8's `.zpkg` (`ZIRK_COMPILER_SPEC.md` section 4): whatever gets fixed here is what cannot be changed later without rebuilding packages that already exist.

This decision also fixes where the line is drawn between "has identity" and "does not have it", because the spec (section 7) requires that a record or a value class **not** have an observable identity, and store only what is strictly necessary ("compact").

## Decision

**An ordinary object is a header plus its fields; a record or value class is just its fields, with no header, passed by value.**

```
   object (class)        =  [ type descriptor | field₁ | field₂ | … ]
   value (record/value class)  =  [ field₁ | field₂ | … ]
```

Two representations in the IR (`IrType::Object(u32)` and `IrType::Value(u32)`), each with its own layout table in the module (`ObjectLayout`/`ValueLayout`) but sharing the same id space as `checked.classes` — `ClassType.kind` decides which one to look at. In LLVM, `llvm_type_in` returns a pointer for `Object` and the struct itself for `Value`: a value travels by value in slots, parameters, return, and as a field of another object or value, with no special calling convention — LLVM already supports aggregates passed by value.

**Inherited fields come before a class's own fields**, in the order the hierarchy declares them. A subclass's layout prefix always matches its superclass's, so accessing an inherited field is the same offset regardless of who's looking at it — single inheritance costs nothing at access time. A record or value class does not support `extends`, so this rule does not apply to it.

**An object's descriptor carries, in order:**

```
[ method table | ancestor count | ancestor₁ … | contract count | (contract, table)… ]
```

- The **method table** is the concrete class's own table, with inherited entries at the same index as in the base (see ADR-013).
- The **ancestors** are the class itself plus each transitive base, in that order — what a checked cast (`as`) walks to decide whether it's valid at runtime.
- Each **satisfied contract** contributes its own table, in the contract's method order — needed because a class implements several contracts and each would want its own indices (see ADR-013).

**A record or value class has no descriptor**, not even an empty one: there is no method table to store (its methods resolve at compile time, never by index — see ADR-013), there is no `extends` that would give it ancestors, and `implements` is blocked with `NOT_LOWERED` precisely because there is nowhere to store a contract's table.

**A generic is specialized, not erased.** `Box<Int32>` and `Box<String>` are two distinct `ObjectLayout`/`ValueLayout` instances, one for each combination of type arguments the program actually uses (roadmap task 11.1) — deduplicated by the checker's own interning. It is the only approach compatible with a generic value class staying inline: erasing types would force everything to go through a pointer.

**An algebraic enum with associated data** (`IrType::Enum(u32)`) is also inline, with its own table (`EnumLayout`): a discriminant plus the payload of **all** variants concatenated, not overlapping like a real union — each variant occupies its own field span. A traditional enum, none of whose variants carries data, remains a plain `Int32`, with no change in representation.

## Rationale

**Header kept separate from the fields, not just one more field.** Whatever Phase 4 needs for memory — marks, counters, whatever the chosen strategy requires — gets added to the header, which exists from the start as its own concept for exactly that purpose. If the descriptor were "the first field" instead of something separate, any future extension would shift the indices of all real fields.

**Ancestors flattened in the header, not a hierarchy walk at runtime.** A checked cast needs to answer "is this the class, or one of its bases?" at runtime without having the full class tree available outside the compiler. Storing the already-flattened list turns that question into a linear search over an array, with no parent pointers to follow.

**Concatenated, not overlapping, variants in an enum.** A real byte-level union would be smaller, but reinterpreting the bytes of the wrong variant is exactly the kind of undefined behavior that `ZIRK_RUNTIME_SPEC.md` promises the safe language does not have. With the typical number of variants at this phase, the space cost isn't tight; this gets revisited if a type with many large-payload variants makes it a real problem.

**A record/value class with no header, not even an empty one.** Paying the size of a header on something the spec requires to be "compact" — and that also travels by value, so its size multiplies on every copy — would be exactly what section 7 prohibits. The consequence of not having one is real and accepted: there is no possible dynamic dispatch on one (see ADR-013), which directly settles the `design.md`'s open question about virtual methods on a value class.

## Consequences

- **`implements` on a record or value class type-checks but does not lower.** Conformance is verified the same way as for an ordinary class, but reaching the method through the contract's type is blocked with `NOT_LOWERED`: there is no descriptor to store its table in. Its own methods, called directly on the concrete type, do dispatch (always statically, never by index).
- **A specialized generic multiplies the generated code and layouts.** Trade-off accepted since D5 of `design.md`: it is the only way to satisfy the inline constraint for generic value classes. The binary-size cost gets measured when Phase 8 addresses that.
- **An object's and a value's layout are contracts with Phase 8's `.zpkg`.** What gets fixed here — inherited field order, header shape, flattened ancestors, concatenated variants — cannot be changed later without invalidating packages that already exist. Whatever Phase 4 needs for memory gets added to the existing header; it should not require redesigning this shape.
- **A value class never reaches a checked cast or an `is` identity check.** Neither makes sense without a header: there is no descriptor to compare and no address that identifies it.
