# ADR-013 — Shape of dispatch

- **Status:** accepted
- **Date:** August 19, 2026
- **Phase:** 3

## Context

Phase 3 brings single inheritance, interfaces, and traits, and with them the first time a method call has no single possible body at compile time: `describe_any(u: User)` might receive a `User` or a `Manager` that overrides it, and `describe(s: Speaker)` doesn't know which concrete class implements `Speaker` until the program runs.

`ZIRK_COMPILER_SPEC.md` section 4 requires the IR to retain enough information to **devirtualize** — which presupposes that there is something to devirtualize, i.e., that not every call pays the same indirection cost.

This decision depends directly on ADR-012: the shape of the descriptor (method table, ancestors, contract tables) is where dispatch lives.

## Decision

**A method call is direct by default; it only goes through a table when the checker proves it's needed, and through a different table depending on how it's called.**

Three forms of a method call, chosen at compile time:

1. **Direct (`InstKind::Call`).** The target is known at compile time: no subclass overrides the method. This is the majority of calls. The receiver travels as the first explicit argument, like any other.
2. **Through the object's own table (`InstKind::CallVirtual`).** The method is overridable — some subclass does override it — and it's called through a type that isn't statically guaranteed to know which body it is. The index in the table is stable between a class and its subclass, because the subclass's table starts with the base's entries in the same order (ADR-012).
3. **Through a contract's table (`InstKind::CallContract`).** The receiver is typed through an interface or trait, not through its own class. Each contract a class satisfies has its own table in the descriptor, in the contract's method order — needed because a class implements several contracts and each would want its own indices if they shared a single table.

**Which method is "overridable" is information the checker already has**, because it knows the crate's full hierarchy before the IR is generated: a method that no declared subclass marks with `override fn` never needs indirection, no matter which type it's called through.

**A record or value class has none of the three indirect forms.** It has no descriptor (ADR-012), so there's no own table or contract table to offer. Its methods always resolve directly — never `overridden`, because neither `record` nor `value class` supports `extends` — and calling one through the type of a contract it implements is blocked with `NOT_LOWERED` at its own declaration.

**A checked cast (`as`) is not method dispatch, but it shares the header.** It verifies at runtime that the object is of the target class or one of its subclasses, walking the descriptor's flattened ancestor list (ADR-012) — a linear search, returning no method.

## Rationale

**Direct by default, not indirect by default.** Paying an indirection on every call would mean paying for generality the program mostly doesn't use: most methods in a typical program are never overridden. The alternative — runtime name-lookup dispatch, in the style of a dynamic language — is simpler to implement and strictly slower on every call, without contributing anything the checker can't already decide at compile time.

**Own table and contract table as separate things, not a combined table.** A class satisfies several contracts at once, each with its own set of methods and its own declaration order. Combining them into a single table would force reserving the widest index across all contracts the class might ever implement, or recomputing indices every time one gets added — neither scales. A table per satisfied contract, indexed by its own order, is what makes **any** class that implements `Speaker` respond to `.speak()` at the same index, regardless of what else it implements.

**No dispatch for a record or value class, not a degraded dispatch.** The alternative — giving it a header just so it can offer a table — directly contradicts ADR-012 and spec section 7 ("compact"). The consequence is accepted: today `implements` on a record type-checks but does not lower: reaching the method through the contract has nowhere to land. The direct call on the concrete type — the vast majority of real record uses — doesn't need any of this.

## Consequences

- **Adding a subclass that overrides an already-compiled method requires recompiling the caller that doesn't know about it.** If `describe_any(u: User)` was compiled assuming direct `describe` before `Manager` existed, recompilation is needed, not incremental linking — consistent with Zirk not having reusable separate object-file compilation yet.
- **Specialized generics (ADR-012) inherit this same rule unchanged**: each instantiation of a generic class has its own method and contract tables, built exactly like a non-generic class's.
- **`for ... in` over a custom `Iterable<T>` uses exactly this mechanism**, with no new one: `iterator()` and `next()` get called through the native `Iterable`/`Iterator` contract's table, the same as any other interface method.
- **The limit documented here — no dispatch for record/value class — is what answers this phase's `design.md` open question about virtual methods**: the answer is no, and this is the structural reason, not a temporary implementation limitation.
