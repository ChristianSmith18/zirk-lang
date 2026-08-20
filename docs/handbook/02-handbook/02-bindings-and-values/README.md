# Bindings and Values

A binding connects a name to a typed value. Zirk makes the permitted kind of change part of the declaration: `mut` allows rebinding, `inmut` fixes the binding, and `inmut::strict` requests transitive immutability.

This unit also covers inference, defaults, initialization, destructuring, shadowing, and the difference between a value's semantics and its physical representation.

## In this unit

1. [`mut`](./01-mut.md)
2. [`inmut`](./02-inmut.md)
3. [Strict Immutability](./03-strict-immutability.md)
4. [Type Inference](./04-type-inference.md)
5. [Default Values](./05-default-values.md)
6. [Definite Initialization](./06-definite-initialization.md)
7. [Destructuring](./07-destructuring.md)
8. [Multiple Bindings and Simultaneous Assignment](./07a-multiple-bindings-and-assignment.md)
9. [Shadowing](./08-shadowing.md)
10. [Value and Reference Semantics](./09-value-and-reference-semantics.md)

---

**Previous:** [← Naming Conventions](../01-program-structure/06-naming-conventions.md) · **Next:** [ mut](01-mut.md)
