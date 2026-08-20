# Shadowing

Zirk rejects local shadowing. A local declaration cannot reuse the name of a still-visible local binding or parameter, even in a nested block.

```zirk
inmut label = "outer";
if detailed {
    inmut label = "detailed"; // Error: `label` is already visible.
}
```

Choose a name that states the new role instead:

```zirk
inmut label = "outer";
if detailed {
    inmut detailed_label = "detailed";
    stdout.println(detailed_label);
}
stdout.println(label);
```

This removes ambiguity about which binding an assignment or capture uses.
Explicit aliases in imports and destructuring remain available through `->`;
they create a different name instead of shadowing an existing one.

Lambda parameters are the deliberate exception for captured outer values: when
a parameter has the same name as a capture, the parameter uses the plain name
and the capture is addressed through `this.name`.

---

**Previous:** [← Multiple Bindings and Simultaneous Assignment](07a-multiple-bindings-and-assignment.md) · **Next:** [ Value and Reference Semantics](09-value-and-reference-semantics.md)
