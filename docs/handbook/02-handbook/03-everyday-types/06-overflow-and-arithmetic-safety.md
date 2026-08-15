# Overflow and Arithmetic Safety

Ordinary integer overflow is a controlled error. Zirk does not let debug and release builds silently disagree about language-level arithmetic.

```zirk
inmut maximum: UInt8 = 255;
inmut invalid = maximum + 1; // controlled overflow
```

Algorithms that intentionally need modular, saturating, or checked behavior must request the relevant explicit operation. Checked operations return a result that forces the caller to handle failure.

The compiler may diagnose constant overflow at compile time. Runtime-dependent overflow must still follow the same observable contract after optimization.

Never use an unsafe cast to disguise an arithmetic policy; select the policy that communicates the algorithm.

---

**Previous:** [← Numeric Literals](./05-numeric-literals.md) · **Next:** [Boolean →](./07-boolean.md)
