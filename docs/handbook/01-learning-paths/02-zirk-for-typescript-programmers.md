# Zirk for TypeScript Programmers

Zirk and TypeScript share readable type annotations, inference, unions, nullable modeling, classes, and familiar expression syntax. Zirk uses structural contracts and traits rather than TypeScript-style interface declarations; the resemblance is useful, but Zirk is not TypeScript with a native backend.

## Recalibrate the execution model

Zirk compiles an application, runtime, standard library, and portable package IR into a standalone native binary. There is no JavaScript host, no Node.js module resolution, no structural assumption that every object shape is interchangeable, and no `undefined` value.

```zirk
inmut label: String = "ready";
mut selected: String? = null;
inmut visible = selected ?? label;
```

`T?` means `T | Null`. Safe access and flow analysis are language rules rather than TypeScript checks erased before runtime.

## Learn the declarations, not just their spelling

`mut`, `inmut`, and `inmut::strict` distinguish reassignment, stable binding, and deep immutability. `inmut` is not simply `const`: it respects the mutability contract of the referenced type. Zirk has nominal classes, structural `contract` declarations, traits, open `Object` values, and algebraic enums; the former `interface` and `record` declarations were removed. Consult each contract rather than assuming TypeScript's structural assignability.

## Replace Promise intuition

Zirk 1.x has no `async fn`; the former `task`, `await`, and `select` syntax was
removed. Structured work uses `concurrent { ... }`, `spawn`, and `Job<T>`;
`Timer` provides the delivered timer operations, while `parallel` requests CPU
work. Typed channels, managed `Thread.run`, scalar atomics, and scoped
`Mutex<T>.with` are available. The general `Thread<T>` lifecycle and other
synchronization primitives remain specified only, and synchronous I/O can
block the cooperative executor.

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
