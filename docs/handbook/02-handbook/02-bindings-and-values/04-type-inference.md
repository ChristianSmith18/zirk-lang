# Type Inference

Zirk infers a type when the available evidence produces one unambiguous answer.

```zirk
inmut title = "Handbook";
mut count = 0;
```

Inference removes repetition; it does not postpone typing until runtime. `title` and `count` have fixed static types, and incompatible later use is rejected.

Write annotations when they express an API contract, select a wider numeric type, document an intended union, or resolve ambiguity:

```zirk
inmut request_id: UInt64 = 42;
mut selected: String? = null;
```

Invalid code should be diagnosed at the point where inference lacks enough information, not silently assigned a catch-all dynamic type. Zirk has no implicit `any` equivalent.

---

**Previous:** [← Strict Immutability](./03-strict-immutability.md) · **Next:** [Default Values →](./05-default-values.md)
