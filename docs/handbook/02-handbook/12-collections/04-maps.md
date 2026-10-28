# Maps

`Map<K, V>` associates unique keys with values. Admitted keys and values are
stored using their complete declared byte layout, including 128-bit integers
and `Float128`; managed references inside supported entries are traced by the
collector. `String`
and `Char` keys are stored as normalized text snapshots, so mutating the source
string after insertion does not change the stored key.

```zirk
mut users: Map<UserId, User> = Map();
users.set(user.id, user);
```

Key types must satisfy the map's equality and hashing contract.
Lookup absence is reported by `get(key): Result<V, LookupError>`. Nullable
value contexts project that result to `V?`; callers that need diagnostics can
match the Result directly.

The current Map preserves insertion order. Updating an existing key keeps its
position; removing and reinserting it places it at the end. Iterating a map
directly yields `Tuple<K, V>` entries in that order. `keys()`, `values()` and
`entries()` return typed insertion-order snapshots.

`K` must meet the map's coherent equality/hash contract. `V` has no blanket capability requirement. Assignment aliases the map; `clone()` creates an independent map according to key/value clone contracts. Insert, update, and removal are referent mutations and are unavailable through strict aliases.

## API

Unique key/value associations; insertion-order iteration; `K` requires coherent
`Hash` + `Equal`.

> The runtime supports typed key and value storage for currently admitted
> scalar key types (all integer widths, binary floats, Decimal, Boolean, text,
> Duration, Date, Time, and DateTime), reference keys, and nullable keys with
> a defined runtime layout, including nullable text. Heterogeneous tuples and
> payload enums derive key equality from recursively eligible, byte-canonical
> fields; embedded `String` and `Char` fields are not yet eligible because
> their equality is content-based. Nullable Decimal, other structural aggregate keys,
> and union keys remain unsupported. Layout-aware Map values and aggregate
> snapshots are implemented; eager transformations are still pending.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `UInt64` | Entry count | implemented |
| `is_empty` | `Boolean` | Whether the map is empty | implemented |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Map()` | `Map<K,V>` | Empty map construction | implemented |
| `Map<K,V>(entries)` | `Map<K,V>` | Construction from entries | specified |
| `m[key]` | `V` | Checked lookup; absence is a typed error | specified |
| `m.get(key)` | `Result<V, LookupError>` (or `V?` in nullable value context) | Safe lookup; preserve or project absence | implemented |
| `m.set(key, value)` | `Void` | Inserts or replaces; replacement retains insertion position | implemented |
| `m.remove(key)` | `Boolean` | Removes the key if present | implemented |
| `m.contains_key(key)` | `Boolean` | Membership | implemented |
| `m.filter(fn)` | `Map<K,V>` | Eager selection over entries `(k, v) -> Bool` | specified |
| `m.map_values(fn)` | `Map<K,U>` | Eager value transform `(k, v) -> U` | specified |
| `m.keys()` | `Array<K>` | Insertion-order snapshot | implemented |
| `m.values()` | `Array<V>` | Insertion-order snapshot | implemented |
| `m.entries()` | `Array<(K, V)>` | Insertion-order snapshot | implemented |
| `m.to_list()` | `List<Tuple(K, V)>` | Materializes all entries | specified |
| `m.to_array()` | `Array<Tuple(K, V)>` | Materializes all entries | specified |
| `m.clear()` | `Void` | Logical emptying | implemented |
| `m.clone()` | `Map<K,V>` | Independent map per key/value clone contracts | specified |
| `m.to_string()` | `String` | Rendering | implemented |

### Examples

```zirk
mut users: Map<UserId, User> = Map();
users.set(user.id, user); // Void
match users.get(id) {
    Error(_) => stdout.println("absent"),
    Ok(user) => stdout.println(user.name),
}

mut labels: Map<String, Int32> = Map();
mut key: String = "café";
labels.set(key, 1); // the collection stores a normalized key snapshot
key[0] = "C"; // this mutation does not move the stored key
```

---

**Previous:** [← Lists](03-lists.md) · **Next:** [ Sets](05-sets.md)
