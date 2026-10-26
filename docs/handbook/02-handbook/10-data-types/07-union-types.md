# Union Types

`A | B` admits a value whose static type is one of the listed alternatives.
Unions accept any number of alternatives. The compiler flattens nested unions,
removes duplicates and `Never`, and removes alternatives already covered by a
wider member. Order and alias spelling do not change the canonical union.

```zirk
fn normalize(value: String | Int32): String {
    return match value {
        "ready" => "text";
        42 => "number";
        _ => "other";
    };
}
```

Code must narrow a union before using operations unavailable on every member.
`T?` is shorthand for `T | Null`.

Before narrowing, only members and operators valid for every alternative with a compatible result are available. `String | Int32` therefore has `to_string()`, but not native `+`. Narrow with `match`, a type pattern, or proven flow analysis; Zirk does not select an alternative dynamically to rescue an invalid operator.

Prefer an enum when alternatives form one owned closed domain with meaningful case names or payloads. Use a union when existing types themselves are the alternatives.

Literal alternatives preserve their exact value and owning type. For example,
`type Status = const(200) | const(404);` accepts only those two `Int32`
values. The same form works for Boolean, String, Char, Decimal, binary Float,
Duration, temporal values, enum variants, tuples, and recursively immutable
value types supported by constant evaluation.

Structural alternatives can use a literal property as a discriminant:

```zirk
type Enabled = { active: true; label: String };
type Disabled = { active: false; reason: String };
type Choice = Enabled | Disabled;
```

When an object literal is checked against `Choice`, the compiler checks each
candidate, uses required properties and literal values to select a unique
alternative, and rejects ambiguous or unmatched literals. Before narrowing,
only compatible properties and methods common to all alternatives are
available. A stable discriminant can narrow a branch; writes or effects that
may mutate the tested path invalidate the refinement.

## API

A union admits a value of any listed alternative; `T?` is `T | Null`. The
compiler normalizes unions (order, duplicates, subsumption, `Never` removal).

### Members

Only members valid for **every** alternative with a compatible result are
available before narrowing:

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| Common member | Shared compatible type | Available when every member provides it | implemented |
| `to_string()` | `String` | Available for value-producing types | implemented |

Narrow with `match`, a type pattern, or proven flow analysis. Zirk does not
select an alternative dynamically to rescue an invalid operator.

### Examples

```zirk
fn normalize(value: String | Int32): String {
    return match value {
        "ready" => "text",
        42 => "number",
        _ => "other",
    };
}
```

---

**Previous:** [← Type Aliases](06-type-aliases.md) · **Next:** [ Class, Contract, or Object?](08-class-or-record.md)
