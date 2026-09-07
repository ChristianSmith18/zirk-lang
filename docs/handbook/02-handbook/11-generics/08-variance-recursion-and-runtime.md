# Variance, Recursion, and Runtime Identity

Generic types are invariant by default. Declare `out T` for a result-only
covariant parameter or `in T` for an input-only contravariant parameter.

```zirk
interface Source<out T> { next(): Iteration<T>; }
interface Sink<in T> { accept(value: T): Void; }
class Cell<T> { mut value: T; } // invariant: reads and writes T
```

The checker validates every occurrence positionally — this verification is
implemented, not just specified. Returning an `in T`, accepting an `out T`,
or exposing either through mutable storage (`mut value: T` forces invariance)
is a compile-time error. Function types inside a generic member retain their
own variance rules.

Recursive constraints are legal when checking reaches a stable solution. A
type cannot contain itself inline without end; managed `Box<T>` supplies finite
indirection at the recursive edge without weakening safety or strictness.

```zirk
enum Tree<T> {
    Empty;
    Node(value: T, left: Box<Tree<T>>, right: Box<Tree<T>>);
}
```

The compiler checks a body once using only declared constraints. Packages keep
portable typed IR; a final build may monomorphize and safely share equivalent
machine code. Runtime identity remains complete, so `List<Int>` and
`List<String>` remain distinct for casts and reflection. Associated types and
higher-kinded parameters are deferred.

Generic extraction follows projection-copy semantics. Returning an element
from a reference container requires `Clone` when `T` can be reference-backed:

```zirk
fn first<T from Clone>(values: List<T>): T { return values[0]; }
```

---

**Previous:** [← Monomorphization](07-monomorphization.md) · **Next:** [Data Types →](../10-data-types/README.md)
