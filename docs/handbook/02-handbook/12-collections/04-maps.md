# Maps

`Map<K, V>` associates unique keys with values.

```zirk
mut users: Map<UserId, User> = Map();
users.set(user.id, user);
```

Key types must satisfy the map's equality and hashing or ordering contract. Lookup absence must remain explicit—commonly through a nullable or result-bearing API—rather than returning a fabricated default.

Iteration order is guaranteed only if the concrete map contract says so. Do not make serialized output depend on unspecified order.

---

**Previous:** [← Lists](./03-lists.md) · **Next:** [Sets →](./05-sets.md)
