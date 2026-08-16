# Generics

Generics express one statically checked algorithm or data structure across a family of types. Constraints state which operations the implementation may use; specialization and monomorphization are compiler strategies that must preserve the source contract.

Use `T from A & B` for multiple requirements. Constraints may name interfaces,
traits, abstract requirement classes, concrete bases, or native contracts.
Inference draws from arguments, receiver, expected result, `Fn` context, and
constraints but never guesses. Explicit arguments resolve ambiguity and only
trailing generic parameters may define defaults.

---

**Previous:** [← Function Types and Callable Values](../07-functions/12-function-types-and-callable-values.md) · **Next:** [Generic Functions →](01-generic-functions.md)
