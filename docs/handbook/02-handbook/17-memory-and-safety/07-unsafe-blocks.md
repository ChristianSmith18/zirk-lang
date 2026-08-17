# `unsafe` Blocks

`unsafe {}` enables a closed set of low-level operations; it does not disable
type checking, scopes, mutability, permissions, or runtime OS validation.

Every block should document the invariant it assumes and the safe contract it returns. Minimize its size so reviewers can audit the entire trust boundary.

Unsafe-only operations are pointer construction/dereference/arithmetic/casts,
unsafe native calls, unchecked native construction, untagged native-union
access, weak atomic ordering, and manual internal safety contracts.

An ordinary block is transactional for managed writes and validated native
ranges:

```zirk
unsafe {
    state.status = "updating";
    mut view = match pointer.as_slice_mut(length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    view[0] = marker;
    match validate(view) {
        Ok(_) => {},
        Error(error) => return Error(error), // restores both writes
    }
}
```

The runtime may implement rollback with first-write journals, copy-on-write,
range snapshots, escape analysis, or static commit proofs. It must preserve the
observable all-or-nothing result on controlled failure.

An `unsafe fn` makes the caller acknowledge its contract but still places each
dangerous implementation expression inside a visible unsafe block.

---

**Previous:** [← Dereferencing](06-dereferencing.md) · **Next:** [ Bounds Safety](08-bounds-safety.md)
