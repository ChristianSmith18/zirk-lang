# Metaprogramming

Zirk metaprogramming is typed, validated, and compile-time only. A decorator declared with `fn dec` inspects one of five supported targets through a public immutable Syntax API, augments compatible declarations, or wraps executable behavior. Its output returns through the complete compiler pipeline.

Decorators are erased after expansion. Frameworks that need runtime information generate ordinary typed descriptors or registries; Zirk does not preserve decorator applications automatically. There is no public compiler AST and no general `comptime {}` block in Zirk 1.x.

Read [Zirk Decorator Semantics](../../DECORATOR_SEMANTICS.md) for the normative contract. This unit explains that contract progressively.

---

**Previous:** [← Why No Inline Assembly?](../05-native-and-low-level/10-why-no-inline-assembly.md) · **Next:** [ Decorators](01-decorators.md)
