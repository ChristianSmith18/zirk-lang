# Why No `new`?

Calling `User(...)` invokes `construct` without promising heap allocation. Omitting `new` keeps allocation strategy separate from domain construction and allows inline or optimized representation where semantics permit.

Classes still have identity; syntax simply avoids encoding a representation decision into every call.

---

**Previous:** [← Why match with?](04-why-match-with.md) · **Next:** [ Why No Traditional Overloading?](06-why-no-traditional-overloading.md)
