# The Resource Contract

`Resource<E from Error>` describes use and typed closure failure for an external
resource:

```zirk
interface Resource<E from Error> {
    fn close(): Result<Void,E>;
    fn is_closed(): Boolean;
}
```

Opening is separate and can fail before a resource exists; closing can fail
after useful work has completed.

The contract guarantees exactly-once closure when managed by `match with`. A garbage collector or object finalizer cannot provide the same deterministic external effect.

Resources do not gain ordinary `Clone`. A type-specific fallible `duplicate`
or `split` operation creates separately closable handles when the platform can
support it.

---

**Previous:** [← Resources](README.md) · **Next:** [ match with](02-match-with.md)
