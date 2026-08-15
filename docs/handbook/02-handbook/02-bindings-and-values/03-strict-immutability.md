# Strict Immutability

`inmut::strict` prevents transitive modification through a binding. Use it for configuration, shared snapshots, and values whose nested state must remain stable.

```zirk
inmut::strict CONFIG: Config = Config();
```

With ordinary `inmut`, the binding cannot point elsewhere, but a method allowed by the underlying mutable type may still change that object. Strict immutability closes that path as well.

The compiler must consider aliases: strictness would be meaningless if another writable reference could mutate the same reachable state without an explicit safe contract. The exact representation remains an implementation decision; strictness is a semantic guarantee, not a promise of deep copying.

Prefer ordinary `inmut` for stable references to intentionally mutable services. Prefer `inmut::strict` for values passed across concurrency boundaries or treated as durable configuration.

---

**Previous:** [← `inmut`](./02-inmut.md) · **Next:** [Type Inference →](./04-type-inference.md)
