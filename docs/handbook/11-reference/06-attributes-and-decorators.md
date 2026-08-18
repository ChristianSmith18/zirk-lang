# Attributes and Decorators

Normative built-in test attributes include `@test`, `@e2e`, and `@bench` in their required file contexts. User decorators use `fn dec`; grouped contiguous applications use `repeatable fn dec`.

The only target blocks are `class`, `attribute`, `function`, `method`, and `parameter`. Expansion uses `match target.transform` with `Inspect`, `Augment`, and `Wrap`; executable wrapping uses `match target.wrap` with `Before`, both `After` forms, `Catch`, or exclusive `Around`. All payload bindings are explicit and `_` consumes exactly one position.

Applications evaluate top-to-bottom and compose with the nearest decorator innermost. `requires decorators [...]`, `before decorators [...]`, and `after decorators [...]` validate different-decorator relationships without reordering. Self-references, cycles, unsupported targets, noncontiguous repeatable groups, public conflicts, and implicit compiler variables are errors.

External effects require approved build-phase permissions. Decorators are erased; runtime needs use explicitly generated ordinary descriptors or registries. See [Zirk Decorator Semantics](../../DECORATOR_SEMANTICS.md).

---

**Previous:** [← Grammar Summary](05-grammar-summary.md) · **Next:** [ Diagnostic Codes](07-diagnostic-codes.md)
