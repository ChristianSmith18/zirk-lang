# Overflow and Arithmetic Safety

Ordinary numeric and temporal arithmetic never wraps silently. A constant
failure is a compile-time diagnostic; a data-dependent failure is a controlled
runtime error with the operation, operands and target type.

```zirk
inmut maximum: UInt8 = 255;
inmut invalid = maximum + 1; // controlled overflow
```

## Explicit policies

Integer families provide three deliberate alternatives for applicable
operations:

- `checked_add(other)` returns a typed result/optional failure;
- `wrapping_add(other)` performs modular arithmetic;
- `saturating_add(other)` clamps to the nearest bound.

The same naming pattern applies to subtraction, multiplication and power where
defined. These methods communicate algorithmic intent and remain identical in
debug and release builds.

## Other arithmetic failures

Division or remainder by zero, invalid shifts, an unrepresentable Float result,
Float indeterminacy, Duration overflow and impossible String repetition
allocation are controlled failures. `unsafe` cannot disguise or redefine the
ordinary policy.

When a conversion may fail, use its checked constructor or parsing result and
handle the error explicitly.

---

**Previous:** [← Numeric Literals](05-numeric-literals.md) · **Next:** [ Boolean](07-boolean.md)
