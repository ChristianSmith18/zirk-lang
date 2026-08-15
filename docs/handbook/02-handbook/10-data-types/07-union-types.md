# Union Types

`A | B` admits a value whose static type is one of the listed alternatives.

```zirk
fn normalize(value: String | Int32): String {
    return match value {
        String(text) => text;
        Int32(number) => "{number}";
    };
}
```

Code must narrow a union before using operations unavailable on every member. `T?` is shorthand for `T | Null`.

Prefer an enum when alternatives form one owned closed domain with meaningful case names or payloads. Use a union when existing types themselves are the alternatives.

---

**Previous:** [← Type Aliases](./06-type-aliases.md) · **Next:** [Class, Record, or Value Class? →](./08-class-record-value-class.md)
