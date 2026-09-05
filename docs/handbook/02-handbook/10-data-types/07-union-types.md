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

Before narrowing, only members and operators valid for every alternative with a compatible result are available. `String | Int32` therefore has `to_string()`, but not native `+`. Narrow with `match`, a type pattern, or proven flow analysis; Zirk does not select an alternative dynamically to rescue an invalid operator.

Prefer an enum when alternatives form one owned closed domain with meaningful case names or payloads. Use a union when existing types themselves are the alternatives.

The compiler normalizes unions: member order is irrelevant, duplicates are
removed, subsumed alternatives collapse, and `Never` disappears when another
member exists. Diagnostics print the canonical form so equivalent unions do
not look different because they were written in another order.

## API

A union admits a value of any listed alternative; `T?` is `T | Null`. The
compiler normalizes unions (order, duplicates, subsumption, `Never` removal).

### Members

Only members valid for **every** alternative with a compatible result are
available before narrowing:

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `type` | `Type` | Universal member | specified — union values are not compilable yet |
| `to_string()` | `String` | Available when every member provides it | specified — union values are not compilable yet |

Narrow with `match`, a type pattern, or proven flow analysis. Zirk does not
select an alternative dynamically to rescue an invalid operator.

### Examples

```zirk
fn normalize(value: String | Int32): String {
    return match value {
        String(text) => text,
        Int32(number) => "{number}",
    };
}
```

---

**Previous:** [← Type Aliases](06-type-aliases.md) · **Next:** [ Class or Record?](08-class-or-record.md)
