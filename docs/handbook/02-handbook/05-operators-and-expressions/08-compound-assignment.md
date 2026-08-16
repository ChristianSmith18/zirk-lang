# Compound Assignment

`+=`, `-=`, `*=`, `/=`, and `%=` combine an operation with assignment.

```zirk
mut total = 0;
total += next_value;
```

The left side must be mutable, and the underlying operator must produce a value assignable to its type. Compound assignment does not authorize an implicit lossy conversion.

Evaluation of an addressable left side occurs according to the language's single-assignment contract; overload implementations cannot change precedence or arity.

---

**Previous:** [← Increment and Decrement](07-increment-and-decrement.md) · **Next:** [ Pipe Operator](09-pipe-operator.md)
