# Pattern Matching

Patterns describe the shape a value must have and bind the data inside that shape. They power `match`, destructuring, type narrowing, and exhaustive decisions over algebraic data.

Both statement and expression matches must be exhaustive for a closed domain.
Zirk has no pattern guards; place conditional logic in the selected branch or
a nested match. Proven unreachable patterns are compile-time errors. Every
binding is an independent projection, so a reference-backed binding is deeply
cloned and requires `Clone`.

1. [Value Patterns](./01-value-patterns.md)
2. [Multiple Patterns](./02-multiple-patterns.md)
3. [Type Patterns](./03-type-patterns.md)
4. [Enum Patterns](./04-enum-patterns.md)
5. [Union Patterns](./05-union-patterns.md)
6. [Destructuring Patterns](./06-destructuring-patterns.md)
7. [`match` as a Statement](./07-match-as-statement.md)
8. [`match` as an Expression](./08-match-as-expression.md)

---

**Previous:** [← Pipelines](../13-iteration-and-functional-style/08-pipelines.md) · **Next:** [ Value Patterns](01-value-patterns.md)
