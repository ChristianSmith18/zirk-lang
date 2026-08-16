# Increment and Decrement

`++` and `--` update a mutable numeric binding by one. Prefix and postfix forms preserve their conventional result timing.

```zirk
mut index = 0;
inmut previous = index++;
inmut current = ++index;
```

Postfix yields the old value before updating; prefix updates first and yields the new value. Use the operators as standalone statements when the yielded value is unnecessary.

They require `mut` and remain subject to overflow rules. Dense expressions that combine multiple updates are legal only when evaluation order is defined, but are usually clearer as separate statements.

---

**Previous:** [← Ternary Operator](06-ternary.md) · **Next:** [ Compound Assignment](08-compound-assignment.md)
