# Type Categories

Zirk types differ in who defines their semantics, whether assignment copies a
value or shares a reference, and which operations the compiler knows.

## Compiler primitives

Primitives have literals or compiler-recognized operators and cannot be
reopened by application code:

```zirk
inmut answer: Int32 = 42;
inmut ratio: Decimal = 0.5;
inmut ready: Boolean = false;
inmut unit: Char = 'π';
```

The primitive families are signed and unsigned integers, `Decimal`, binary
`Float16` through `Float128`, `Boolean`, `Char`, and exact `Duration`. Assignment
copies them:

```zirk
mut first = 7;
mut second = first;
second = 8; // first stays 7
```

## Native value types

`Date`, `Time`, `DateTime`, `Duration`, `Instant`, `ZonedDateTime`, `TimeZone`,
`Period`, and `TimeShift` are delivered immutable temporal values. The zone and
locale family is still partial: its implemented IANA/fixed-offset operations,
localized formatting and parsing, and DST policies are listed in the
[temporal status reference](../../11-reference/12-feature-status.md); the
cross-host execution matrix remains open. Transformations return new values
where those APIs exist.

Tuples and enums are also value aggregates. Their value equality and cloning
follow their declared component capabilities.

## Reference types

`String`, collections, `Object`, and class instances have reference semantics.
Assignment normally aliases the same instance. Binding qualifiers control
rebinding and mutation:

```zirk
mut first = "hello";
mut second = first;
second[0] = 'H'; // both observe "Hello"
```

A dynamic object's nested object property preserves reference identity when
read. `clone()` creates an independent graph only when the relevant type
implements `Clone`.

## User-defined structural and nominal types

- A `contract` declares structural property and operation requirements.
- `Object` is an open dynamic object; an object literal may add properties.
- A `class` is nominal, referenced, stateful, and has observable identity.
- An enum is a closed nominal set, optionally with associated values.
- An alias names an existing type; a union lists explicit alternatives.

```zirk
contract Named { name: String; }

inmut user: Object = { name: "Ada", active: true };
inmut view: Named = user;

class Counter { value: Int32; }
enum Status { idle; running; done; }
```

A contract view preserves the object's identity and exposes only its declared
members. Successful conformance seals the object's current keys across aliases.
Objects and classes do not infer structural equality; `is` compares identity.

## Special, borrowed, and unsafe types

`Dynamic`, `Null`, `Void`, and `Never` describe tagged runtime values, absence,
no-value completion, and non-returning control flow. Pointers and borrowed
views have their own lifetime and safety rules.

---

**Previous:** [← Object and Type Hierarchy](01-object-and-type-hierarchy.md) · **Next:** [Value and Reference Behavior](01b-value-and-reference-behavior.md)
