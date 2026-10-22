# Maps

`Map<K, V>` associates unique keys with values. The compiler currently provides
a restricted runtime representation: keys and values occupy one machine word,
so semantic analysis rejects aggregate shapes that do not fit that layout.
Managed references in supported entries are traced by the collector. `String`
and `Char` keys are stored as normalized text snapshots, so mutating the source
string after insertion does not change the stored key.

```zirk
mut users: Map<UserId, User> = Map();
users.set(user.id, user);
```

Key types must satisfy the map's equality and hashing or ordering contract.
Lookup absence is reported by `get(key): Result<V, LookupError>`. Nullable
value contexts project that result to `V?`; callers that need diagnostics can
match the Result directly.

Iteration order is guaranteed only if the concrete map contract says so. Do not make serialized output depend on unspecified order.

`K` must meet the selected equality/hash or ordering contracts. `V` has no blanket capability requirement. Assignment aliases the map; `clone()` creates an independent map according to key/value clone contracts. Insert, update, and removal are referent mutations and are unavailable through strict aliases.

## API

Unique key/value associations; insertion-order iteration; `K` requires coherent
`Hash` + `Equal`.

> The API below is the language contract. The current compiler implements only
> the basic constructor, length/emptiness, set, contains, nullable lookup, and
> remove operations described in the feature-status table; iteration and the
> remaining methods are still specified rather than delivered.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `UInt64` | Entry count | specified |
| `is_empty` | `Boolean` | `length == 0` | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Map()` / `Map<K,V>(entries)` | `Map<K,V>` | Construction | specified |
| `m[key]` | `V` | Checked lookup; absence is a typed error | specified |
| `m.get(key)` | `Result<V, LookupError>` (or `V?` in nullable value context) | Safe lookup; preserve or project absence | implemented |
| `m.set(key, value)` | `SetResult<V>` | `SetResult.Inserted` or `SetResult.Replaced(previous)` | specified |
| `m.remove(key)` | `V?` | Removes and returns the value if present | specified |
| `m.contains_key(key)` | `Boolean` | Membership | specified |
| `m.filter(fn)` | `Map<K,V>` | Eager selection over entries `(k, v) -> Bool` | specified |
| `m.map_values(fn)` | `Map<K,U>` | Eager value transform `(k, v) -> U` | specified |
| `m.keys()` | `Iterator<K>` | Key view in insertion order | specified |
| `m.values()` | `Iterator<V>` | Value view in insertion order | specified |
| `m.entries()` | `Iterator<Tuple(K, V)>` | Entry view | specified |
| `m.to_list()` | `List<Tuple(K, V)>` | Materializes all entries | specified |
| `m.to_array()` | `Array<Tuple(K, V)>` | Materializes all entries | specified |
| `m.clear()` | `Void` | Logical emptying | specified |
| `m.clone()` | `Map<K,V>` | Independent map per key/value clone contracts | specified |
| `m.to_string()` | `String` | Rendering | specified |

### Examples

```zirk
mut users: Map<UserId, User> = Map();
inmut outcome = users.set(user.id, user);
match users.get(id) {
    Error(_) => stdout.println("absent"),
    Ok(user) => stdout.println(user.name),
}

mut labels: Map<String, Int32> = Map();
mut key: String = "café";
labels.set(key, 1);        // the collection stores a normalized key snapshot
key[0] = "C";              // this mutation does not move the stored key
```

---

**Previous:** [← Lists](03-lists.md) · **Next:** [ Sets](05-sets.md)
