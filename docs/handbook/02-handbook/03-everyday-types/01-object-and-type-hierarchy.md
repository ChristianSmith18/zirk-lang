# Object and Zirk's Type Families

`Object` is not a universal root type. It is the common reference-object view
for dynamic objects and class instances. Scalars, tuples, enums, and functions
do not widen to `Object`; use a generic type parameter when an API should
preserve any input type, or `Dynamic` when it should hold differently typed
values for checked runtime use.

```text
Zirk values
├── Scalars and temporal values
├── Tuples and enums
├── Reference objects
│   ├── Object (open dynamic properties)
│   └── Class instances (declared fields + Object view)
├── Collections and callables
└── Special: Dynamic, Null, Void, Never, pointers, views, tasks
```

This is a learning map, not a subtype graph. Numeric capabilities do not make
`Int32` interchangeable with `Decimal`; date and duration operations remain
limited to the combinations their types define.

## Object values

An `Object` stores string-keyed dynamic properties. Its identity is stable, and
its properties may be added, updated, read, or removed subject to Zirk's
mutability rules. Unknown reads produce `Dynamic`; narrow the value before using
it as a concrete type. Class instances share this `Object` view while retaining
their declared class fields and methods.

A `contract` names required properties and operations. A successful check of a
dynamic object against a contract validates its shape and seals the object's
existing key set across aliases. `Dynamic` remains a tagged value, not a parent
class for every runtime type.

## Semantics are not layout

A scalar, tuple, enum, or temporal value may be stored inline. A `String`,
collection, class instance, and dynamic `Object` have reference semantics. The
compiler may specialize, box, cache, or allocate values when observable
behavior remains unchanged. Only an ABI document makes physical layout part of
a public contract.

Continue with [How Values Live and Share](./00-how-values-live-and-share.md)
for assignment and mutation behavior, then [Type Categories](./01a-type-categories.md)
before choosing an individual built-in type.

---

**Previous:** [← How Values Live and Share](00-how-values-live-and-share.md) · **Next:** [Type Categories](01a-type-categories.md)
