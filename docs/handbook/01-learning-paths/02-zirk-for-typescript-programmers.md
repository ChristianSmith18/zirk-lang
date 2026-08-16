# Zirk for TypeScript Programmers

Zirk and TypeScript share readable type annotations, inference, unions, nullable modeling, classes, interfaces, and familiar expression syntax. The resemblance is useful, but Zirk is not TypeScript with a native backend.

## Recalibrate the execution model

Zirk compiles an application, runtime, standard library, and portable package IR into a standalone native binary. There is no JavaScript host, no Node.js module resolution, no structural assumption that every object shape is interchangeable, and no `undefined` value.

```zirk
inmut label: String = "ready";
mut selected: String? = null;
inmut visible = selected ?? label;
```

`T?` means `T | Null`. Safe access and flow analysis are language rules rather than TypeScript checks erased before runtime.

## Learn the declarations, not just their spelling

`mut`, `inmut`, and `inmut::strict` distinguish reassignment, stable binding, and deep immutability. `inmut` is not simply `const`: it respects the mutability contract of the referenced type. Zirk has nominally declared classes, interfaces, traits, records, and algebraic enums; consult each contract rather than assuming TypeScript's structural assignability.

## Replace Promise intuition

Zirk 1.x deliberately has no `async fn`. `task` and `await` express structured concurrent work, `parallel` expresses multicore CPU work, and `thread` names an OS thread. Cancellation and child lifetimes belong to the scope, not to an ambient event loop.

## Replace exception-only failure intuition

Expected failure uses `Result<T, E>` and exhaustive `match`. Exceptions remain for exceptional recoverable conditions, while `fatalError` terminates an irreparable state. Resources add `match with`, which guarantees closure across return, failure, exception, and cancellation.

Zirk callable annotations use `Fn(P...) => R`, not TypeScript's arrow type
syntax. Objects expose attributes and ordinary `get_`/`set_` methods rather
than a `property` construct. Assigning a complete reference aliases it, but
reading an attribute, index, slice, or destructured part returns an independent
deep projection; this differs from JavaScript's usual nested-reference sharing.

## Recommended route

Read bindings, the type model, nullability, classes and traits, algebraic enums, errors, resources, concurrency, modules, and project permissions. Then study packages: `.zpkg` carries typed public API and portable IR, not JavaScript source plus declaration files.

---

**Previous:** [← Zirk for New Programmers](01-zirk-for-new-programmers.md) · **Next:** [ Zirk for Python Programmers](03-zirk-for-python-programmers.md)
