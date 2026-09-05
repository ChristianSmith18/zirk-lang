# Built-in Types

This catalog is the quick lookup for Zirk's compiler-known types. For semantics,
examples, and advice on choosing a type, begin with
[How Values Live and Share](../02-handbook/03-everyday-types/00-how-values-live-and-share.md).

## Type catalog

| Category | Types | Storage/behavior | Default |
|---|---|---|---|---|
| Signed integer | `Int8`, `Int16`, `Int32`, `Int64`, `Int128`; `Int` = `Int32` | value, fixed-width, checked arithmetic | `0` |
| Unsigned integer | `UInt8`, `UInt16`, `UInt32`, `UInt64`, `UInt128`; `UInt` = `UInt32` | value, fixed-width, checked arithmetic | `0` |
| Floating point | `Float16`, `Float32`, `Float64`, `Float128`; `Float` = `Float64` | IEEE-style binary value without valid `NaN` | `0.0` |
| Logic | `Boolean` | value; only `true` or `false` | `false` |
| Text unit | `Char` | one Unicode grapheme value | invalid without an explicit value |
| Text | `String` | mutable, shared reference; grapheme-indexed | `""` |
| Temporal values | `Date`, `Time`, `DateTime`, `Instant`, `ZonedDateTime`, `TimeZone`, `Duration`, `Period` | immutable value types; nanosecond precision where applicable | type-specific or explicit |
| Collections | `Array<T>`, `Array<T, N>`, `List<T>`, `Map<K, V>`, `Set<T>`, `Range<T>` | native reference containers except value-like `Range` | empty where meaningful |
| Callable | `Function(P...) => R`; `Fn(P...) => R` | signature-compatible callable identity with compiler-managed environment | no implicit default |
| Product | `Tuple(T...)` | immutable heterogeneous value with constant `[N]` access | component defaults where explicitly constructed |
| Managed observation | `Weak<T>` | non-owning managed reference; upgrades through `T?` | no implicit default |
| Native memory | `Pointer<T>`, `NativeSlice<T>`, `NativeSliceMut<T>` | unsafe raw address or bounded dependent native view | null only for raw pointer |
| Concurrency | `Task<T>`, `TaskSettlement<T>`, `Channel<T>`, `Thread<T>` | scoped execution and typed coordination | no implicit default |
| Synchronization | `Mutex<T>`, `RwLock<T>`, `Semaphore`, `Barrier`, `Once<T>`, `Atomic<T>` | contract-controlled shared state | type-specific |
| Special | `Null`, `Void`, `Never`, `Object` | absence, no result, no return, and semantic root | varies |

`Decimal*` is not a Zirk type family. Exact base-ten arithmetic may later be supplied as a distinct standard-library type; it must not be confused with `Float`.

## Examples by category

### Immediate values

```zirk
inmut count: Int32 = 0;
mut temperature: Float64 = 98.6;
inmut flag: Boolean = true;
inmut unit: Char = 'π';
```

See [Signed Integers](../02-handbook/03-everyday-types/02-signed-integers.md),
[Floats](../02-handbook/03-everyday-types/04-decimals.md),
[Boolean](../02-handbook/03-everyday-types/07-boolean.md), and
[Char](../02-handbook/03-everyday-types/08-char.md).

### Native values

```zirk
inmut timeout: Duration = 250ms;
inmut today: Date = Date(2026, 9, 5);
```

See [Duration](../02-handbook/03a-temporal/08-duration.md),
[Date](../02-handbook/03a-temporal/02-date.md), and the other temporal types.

### Managed references

```zirk
mut text: String = "hello";
mut numbers: List<Int32> = List(1, 2, 3);
mut buffer: Array<Byte> = [0x01, 0x02, 0x03];
```

See [String](../02-handbook/03-everyday-types/09-string.md),
[Lists](../02-handbook/12-collections/03-lists.md), and
[Arrays](../02-handbook/12-collections/01-arrays.md).

### User-defined values and references

```zirk
record Point { x: Float64; y: Float64; }
class Session { token: String; }
```

See [Records](../02-handbook/10-data-types/01-records.md),
[Class or Record?](../02-handbook/10-data-types/08-class-or-record.md).

### Borrowed and unsafe

```zirk
mut weak = Weak.from(cache);
unsafe { mut raw: Pointer<Int32> = ...; }
```

See [Safe References](../02-handbook/17-memory-and-safety/04-safe-references.md)
and [Pointers and Native Slices](../02-handbook/17-memory-and-safety/05-pointers.md).

## Domain types

User programs add nominal reference `class` types, immutable structural `record`
values, traditional and algebraic `enum` values, aliases, and unions. These are
not compiler primitives, but all belong under the conceptual `Object` root.

Complete reference variables alias when moved. Reads through an attribute,
index, slice, destructuring, pattern, iterator, argument, return, or closure
capture are independent projections and require `Clone` when reference-backed.

`Task<T>` awaits to exactly `T`. `TaskSettlement<T>` is
`Fulfilled(T) | Rejected(Throwable) | Cancelled(CancelledError)`. Internal
transfer/share properties are compiler-derived rather than user-declared types.

## Universal and conditional members

Every inhabited value exposes `type` and `to_string()`. Equality, hashing, ordering, iteration, indexing, arithmetic, and cloning exist only when the type satisfies the corresponding capability contract. Native behavior is closed: programs cannot reopen a built-in type or replace its operator meaning.

## Choosing a type

- Use `Int` for ordinary whole-number counters; pick an explicit width at storage, ABI, or protocol boundaries.
- Use `UInt*` only when the domain and boundary genuinely require a non-negative binary integer.
- Use `Float` for approximate real-number computation, never for exact money.
- Use `Char` for one user-perceived Unicode grapheme and `String` for text.
- Choose temporal types by meaning, not formatting: a birthday is a `Date`, an elapsed timeout is a `Duration`, and a globally scheduled event is a `ZonedDateTime` or `Instant`.
- Use `List<T>` when the size changes, `Array<T>` or `T[n]` when it is fixed, `Pointer<T>` only inside `unsafe`, and `Weak<T>` when you do not want to keep an object alive.

See also [Type Member Index](./13-type-member-index.md), [Temporal Reference](./14-temporal-reference.md), and [Choosing a Type](../02-handbook/03-everyday-types/01f-choosing-a-type.md).

---

**Previous:** [← Operators and Precedence](02-operators-and-precedence.md) · **Next:** [ Literals](04-literals.md)
