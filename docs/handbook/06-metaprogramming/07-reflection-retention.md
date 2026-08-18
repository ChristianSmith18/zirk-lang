# Decorator Erasure and Generated Descriptors

Decorator retention is not a Zirk 1.x feature. The compiler keeps validated generated behavior and removes the decorator machinery. This makes binaries smaller, startup static, and framework contracts visible during compilation.

When runtime data is truly needed, the decorator must generate an ordinary descriptor explicitly. That descriptor contains only library-defined typed data and cannot reveal private structure or grant mutation by implication. Public generated descriptors appear in `public.api`, documentation, compatibility checks, and tooling; private unused descriptors can be eliminated.

---

**Previous:** [← Runtime Reflection](06-runtime-reflection.md) · **Next:** [ Generated Diagnostics](08-generated-diagnostics.md)
