# Strict Immutability

`inmut::strict` prevents transitive modification through a binding. Use it for configuration, shared snapshots, and values whose nested state must remain stable.

```zirk
inmut::strict CONFIG: Config = Config();
```

With ordinary `inmut`, the binding cannot point elsewhere, but a method of the underlying type may still change that object. Strict immutability closes that path as well: the compiler infers which methods mutate their receiver (a write to `this.*`, or a call to another inferred-mutating `this` method), and calling an inferred-mutating method on an `inmut::strict` receiver is an error whose diagnostic shows the mutation chain. Plain `inmut` receivers keep no such restriction.

The compiler must consider aliases: strictness would be meaningless if another writable reference could mutate the same reachable state without an explicit safe contract. The exact representation remains an implementation decision; strictness is a semantic guarantee, not a promise of deep copying.

```zirk
inmut::strict frozen = "hello";
mut alias = frozen; // error: would create a mutable alias
mut copy = frozen.clone(); // valid independent String
```

The reverse acquisition is also restricted: `inmut::strict frozen = mutable`
is invalid while the mutable alias remains accessible. This is an alias
invariant, not merely a read-only view.

Prefer ordinary `inmut` for stable references to intentionally mutable services. Prefer `inmut::strict` for values passed across concurrency boundaries or treated as durable configuration.

---

**Previous:** [← inmut](02-inmut.md) · **Next:** [ Type Inference](04-type-inference.md)
