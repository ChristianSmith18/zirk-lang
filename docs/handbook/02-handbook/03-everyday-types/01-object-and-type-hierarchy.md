# Object and Type Hierarchy

Every value in Zirk belongs semantically to a class. This gives fundamental values a consistent place in generic constraints and method contracts without requiring every value to allocate an object on the heap.

`Object` is the broad root abstraction. More specific types preserve stronger operations: integers support arithmetic, `Boolean` supports logical operators, and `String` provides Unicode text behavior.

Static typing means the compiler uses the narrowest established type when validating an operation. Widening a value to `Object` loses specific operations until safe narrowing restores them.

Do not confuse semantic hierarchy with representation. ABI layout, boxing, stack placement, and specialization remain compiler decisions unless a native boundary documents them.

---

**Previous:** [← Everyday Types](./README.md) · **Next:** [Signed Integers →](./02-signed-integers.md)
