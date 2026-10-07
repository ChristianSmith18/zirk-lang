## Context

Natively, `.clone()` lowers to one `InstKind::Clone` over an `Object` or `Callable`, while a `record` (`IrType::Value`) rebuilds itself structurally in lowering. The native runtime (`crates/zirk-runtime/src/clone.rs`) walks the collector's per-class field-offset table with a memoization map keyed by source address, records each destination before populating its fields (so cycles terminate), and registers in-progress destinations as collector roots.

The interpreter has no collector roots to manage: values live in a bounded arena addressed by stable identifiers, and `Weak<T>` is excluded. Compile-time checking already guarantees that only graphs made of `Clone`-able members reach this instruction.

## Goals / Non-Goals

**Goals:**
- Native-equivalent deep clone semantics on the interpreter heap, including shared children and cycles.
- Deterministic, bounded cost under the existing limits.

**Non-Goals:**
- Enabling `Weak<T>`, pointer, resource or journal behavior.
- Changing which types the checker accepts as `Clone`.
- Sharing the native traversal code verbatim; the two engines use different heap representations.

## Decisions

1. **Iterative worklist with an identifier map.** The interpreter copies the graph with an explicit worklist and a `source HeapId -> destination HeapId` map, allocating and recording each destination before filling its fields. Alternative considered: recursive Rust functions mirroring native — rejected because deep chains would overflow the host stack inside a worker, and the interpreter already avoids host recursion for Zirk calls.
2. **Layout-driven field walk.** The walk reads field kinds from the module's object layouts (the same layouts `LoadField`/`StoreField` use) and treats scalars, nullable scalars, strings and `Value` fields as copy-by-value, and object, closure, `Callable` and collection references as graph edges. Strings and immutable values keep their existing identity semantics from the interpreter's string rules, so cloning does not break `is` expectations relative to native.
3. **Charge work before allocating.** Each cloned object adds a fixed cost to the instruction counter proportional to its field count, and every allocation goes through the existing weighted heap accounting. Exceeding either yields the existing `limit-exceeded` result naming `instructions` or `heap`; a partially built clone is discarded. Alternative considered: an uncharged clone — rejected because a cyclic or wide graph could otherwise bypass the budget.
4. **Collections clone element-wise.** `List`, `Array`, `Map` and `Set` fields clone to new containers whose elements follow the same memo map, matching the deep semantics documented for the object graph. `ArrayClone`/`ListClone` instructions (shallow, already supported) are unchanged.
5. **Preflight reclassification is data-driven.** `Clone` moves to browser-supported in the same commit that adds passing fixtures, so the support matrix test that checks exhaustiveness stays green.

## Risks / Trade-offs

- [Divergence from native on shared immutable values such as strings] → Fixtures assert `is` on cloned string fields and compare to native output; any difference is resolved by matching native semantics.
- [Large graphs hit the heap limit mid-clone] → Work is charged incrementally and the clone is atomic from the program's view; a limit failure leaves the source graph untouched.
- [Subclass polymorphism: static type is a supertype] → The copy reads the runtime class layout from the object header equivalent, not the static type; a fixture clones a `Dog` through an `Animal` reference.
- [Callable clone semantics are subtle] → Mirror native `zirk_rt_clone_callable`: captured environments follow the memo map; fixtures cover a closure capturing a mutable object.
