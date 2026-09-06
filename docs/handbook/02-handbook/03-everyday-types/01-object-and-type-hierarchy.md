# Object and the Type Tree

Every Zirk value has a static type and belongs conceptually beneath `Object`.
That common root lets generic APIs ask for universal operations such as
`type` and `to_string()` without claiming that every value is a heap object.

```text
Object
├── Value
│   ├── Numeric
│   │   ├── Integer: Int8 … Int128, UInt8 … UInt128
│   │   ├── Decimal (exact base-ten decimal)
│   │   └── Float: Float16 … Float128
│   ├── Boolean
│   ├── Char
│   ├── Temporal
│   ├── Record
│   └── Enum
├── Reference
│   ├── String
│   ├── Array, List, Map, Set
│   └── Class
└── Special
    ├── Null
    ├── Void
    └── Never
```

The tree is a learning model, not automatic subtype permission. `Int32` and
`Float` share numeric capabilities, but a function accepting `Int32` does not
accept every numeric value. `Date` and `Duration` are both temporal, but only
the combinations defined by their contracts compile.

## Semantics are not layout

An `Int32`, record, or temporal value may be stored inline. A
`String`, collection, or class instance has reference semantics. The compiler
may still specialize, move, box, cache, or allocate values when observable
behavior remains unchanged. Only an ABI document can make physical layout part
of a public contract.

## Static type controls available operations

```zirk
inmut count: Int32 = 3;
inmut broad: Object = count;
```

`count` supports integer arithmetic. `broad` exposes only operations guaranteed
by `Object` until a safe check narrows it. Zirk never searches for an operation
dynamically merely because the runtime value happens to support it.

Continue with [How Values Live and Share](./00-how-values-live-and-share.md) for
the memory-behavior spectrum, then [Type Categories](./01a-type-categories.md)
before choosing an individual built-in type.

---

**Previous:** [← How Values Live and Share](00-how-values-live-and-share.md) · **Next:** [ Type Categories](01a-type-categories.md)
