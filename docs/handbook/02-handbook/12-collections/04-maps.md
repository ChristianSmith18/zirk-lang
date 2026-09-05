# Maps

`Map<K, V>` associates unique keys with values.

```zirk
mut users: Map<UserId, User> = Map();
users.set(user.id, user);
```

Key types must satisfy the map's equality and hashing or ordering contract. Lookup absence must remain explicit—commonly through a nullable or result-bearing API—rather than returning a fabricated default.

Iteration order is guaranteed only if the concrete map contract says so. Do not make serialized output depend on unspecified order.

`K` must meet the selected equality/hash or ordering contracts. `V` has no blanket capability requirement. Assignment aliases the map; `clone()` creates an independent map according to key/value clone contracts. Insert, update, and removal are referent mutations and are unavailable through strict aliases.

## API

Unique key/value associations; insertion-order iteration; `K` requires coherent
`Hash` + `Equal`.

> Entirely `specified` — `Map` is ahead of the current compiler.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `length` | `Int32` | Entry count | specified |
| `is_empty` | `Boolean` | `length == 0` | specified |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `Map()` / `Map<K,V>(entries)` | `Map<K,V>` | Construction | specified |
| `m[key]` | `V` | Checked lookup; absence is a typed error | specified |
| `m.get(key)` | `Result<V, LookupError>` | Result-bearing lookup | specified |
| `m.get_or_null(key)` | `V?` | Nullable lookup | specified |
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
match users.get_or_null(id) {
    null => stdout.println("absent"),
    user => stdout.println(user.name),
}
```

---

**Previous:** [← Lists](03-lists.md) · **Next:** [ Sets](05-sets.md)
