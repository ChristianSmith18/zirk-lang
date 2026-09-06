# Type Categories

Zirk classifies types by who defines their semantics and whether assignment
copies a value or shares a reference. These categories help predict mutation,
identity, conversion, and which operations the compiler knows intrinsically.

The five memory positions from [How Values Live and Share](00-how-values-live-and-share.md)
map directly onto the categories below.

## Compiler primitives

Primitives have literals or compiler-recognized operators, may be represented
inline, and cannot be reopened by application code:

```zirk
inmut answer: Int32 = 42;
inmut ratio: Float = 0.5;
inmut ready: Boolean = false;
inmut unit: Char = 'π';
```

- signed and unsigned integers;
- `Float` (exact base-ten decimal) and the binary `Float16` … `Float128`;
- `Boolean`;
- `Char`;
- exact `Duration`.

A primitive still has properties, methods, and contracts. Calling
`value.abs()` does not imply observable boxing. Assignment always copies:

```zirk
mut first = 7;
mut second = first;
second = 8;

// first is still 7
```

## Native value types

The language or standard library defines these immutable semantic values, but
they are not all lexical primitives: `Date`, `Time`, `DateTime`, `Instant`,
`ZonedDateTime`, `TimeZone`, `Period`, and supporting temporal enums. Their
transformations return new values.

```zirk
inmut today = Date(2026, 9, 5);
inmut tomorrow = today.add(Duration(days: 1));

// today and tomorrow are independent values
today == tomorrow; // false
```

Records and tuples are user-declared value types, not native, but they share
the same copy-on-assignment behavior:

```zirk
record Point { x: Float; y: Float; }

inmut a = Point(x: 1.0, y: 2.0);
inmut b = a;
// b is an independent copy
```

## Native reference types

`String`, `Array<T>`, `List<T>`, `Map<K,V>`, and `Set<T>` are managed
references. Assignment normally aliases the same instance. Binding qualifiers
decide whether a name can be rebound or the referent mutated.

```zirk
mut first = "hello";
mut second = first;
second[0] = 'H';

// both observe "Hello"
```

A `clone()` makes an independent logical copy when the type implements `Clone`:

```zirk
mut independent = first.clone();
independent[0] = 'Y';

// first is "Hello", independent is "Yello"
```

## User-defined types

- A `class` is nominal, referenced, stateful, and has observable identity.
- A `record` is nominal, immutable, and structurally equal by its fields.
- An enum is a closed nominal set, optionally with associated values.
- An alias names an existing type; a union lists explicit alternatives.

```zirk
class Counter {
    value: Int32;
}

record Settings {
    debug: Boolean;
}

enum Status {
    idle;
    running;
    done;
}

inmut a = Counter(); // a reference with identity
inmut b = Settings(debug: true); // an independent value
inmut c = Status.running; // an enum case
```

Two records with the same fields are equal; two classes are only `==` if their
equality contract says so, and `is` only if they are the same instance.

## Special types

`Null`, `Void`, and `Never` describe absence, normal no-value completion, and
non-returning control flow. They are not ordinary data containers.

```zirk
mut selected: String? = null;     // selected is String | Null
fn log_ready(): Void { }          // completes without a value
fn fail(): Never { fatalError("..."); } // never completes normally
```

## Borrowed and unsafe types

Some types represent *views* or *addresses* rather than owned values:

- `Weak<T>` observes an object without keeping it alive.
- `Dependent<T>` and `NativeSlice<T>` are bounded views tied to a larger owner.
- `Pointer<T>` is an unsafe raw address.

```zirk
mut cache = "expensive";
mut weak = Weak.from(cache);

match weak.upgrade() {
    null => stdout.println("gone"),
    live => stdout.println(live),
}

unsafe {
    mut raw: Pointer<Int32> = ...;
    raw.write(42);
}
```

Borrowed views do not own the memory they read. Pointers own nothing at all.

## Choosing a category

Use the narrowest type that expresses the domain. A user ID should be a record
or class rather than a loose integer; a birthday is a `Date`, not an `Instant`;
a timeout is a `Duration`, not an integer count of milliseconds; growing text is
a `String`, while binary protocol data is bytes.

See [Choosing a Type](01f-choosing-a-type.md) for concrete decision rules.

---

**Previous:** [← Object and the Type Tree](01-object-and-type-hierarchy.md) · **Next:** [ Value and Reference Behavior](01b-value-and-reference-behavior.md)
