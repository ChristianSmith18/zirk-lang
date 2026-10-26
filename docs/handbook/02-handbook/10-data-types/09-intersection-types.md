# Intersection Types

`A & B` describes a value that satisfies every constituent type. Intersections
are static views over the same value; they do not copy it or add a runtime
header. `&` binds more tightly than `|`, and parentheses can change grouping.

```zirk
contract Named { name: String; }
contract Identified { id: Int32; }
type Entity = Named & Identified;

class Person implements Named, Identified {
    name: String;
    id: Int32;
    construct(name: String, id: Int32) {
        this.name = name;
        this.id = id;
    }
}

fn identify(value: Entity): Int32 {
    return value.id;
}
```

The checker combines only members proven by the constituents. A property read
uses its resolved type; conflicting requirements or incompatible mutability
make the intersection invalid. An intersection does not prove that a value is
an unrelated nominal class or enum.

Normalization flattens nested intersections, removes duplicates, and is
independent of source order. Type constraints use the same operator, so
`T from Readable & Closeable` requires both capabilities.

Use a union (`A | B`) when a value may be one of several alternatives. Use an
intersection when the same value must meet multiple compatible requirements.

---

**Previous:** [← Class, Contract, or Object?](08-class-or-record.md) · **Next:** [Collections →](../12-collections/README.md)
