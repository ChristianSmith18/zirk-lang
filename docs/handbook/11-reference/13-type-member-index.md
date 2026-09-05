# Type Member Index

This is a lookup index, not a substitute for each explanatory chapter. Properties have no parentheses; methods do.

| Type/family | Properties | Principal methods | Full explanation |
|---|---|---|---|
| signed/unsigned integers | `min`, `max`, `bits`, `is_even`, `is_odd` | `abs()`, `count_ones()`, `leading_zeros()`, `rotate_left()`, checked/wrapping/saturating operations, `to_string()` | [Signed](../02-handbook/03-everyday-types/02-signed-integers.md), [unsigned](../02-handbook/03-everyday-types/03-unsigned-integers.md) |
| Float | `min`, `max`, `infinity`, `negative_infinity` | `abs()`, `floor()`, `ceil()`, `round()`, `trunc()`, `fract()`, `is_finite()`, `is_infinite()`, conversions | [Floats](../02-handbook/03-everyday-types/04-decimals.md) |
| Boolean | — | `to_string()` | [Boolean](../02-handbook/03-everyday-types/07-boolean.md) |
| Char | byte/code-point metadata | `ascii_code()`, classification, normalization, `to_uppercase()`, `to_lowercase()`, `to_string()` | [Char](../02-handbook/03-everyday-types/08-char.md) |
| String | `length`, byte/code-point counts | search, prefix/suffix, replacement, trim, case, split, lines, substring, normalization, grapheme/code-point/byte views, `clone()`, `to_string()` | [String](../02-handbook/03-everyday-types/09-string.md) |
| temporal family | component and identity properties | construction, parse/format, arithmetic, replacement, boundary and zone operations | [Temporal Reference](./14-temporal-reference.md) |
| collections | `length`, `is_empty` | access, mutation, cloning, iteration and type-specific search/update operations | [Collections](../02-handbook/12-collections/README.md) |
| callable values | — | invocation, `clone()`, `to_string()`; identity via `is` | [Function Types](../02-handbook/07-functions/12-function-types-and-callable-values.md) |
| `Regex` | pattern | `matches()`, `find()`, `replace()`, `split()`; `parse()` for dynamic patterns; `Regex.Match` exposes `group()`/`start`/`end`/`text` | [`std.text`](../04-standard-library/05a-std-text.md) |
| `Result<T,E>` | variant state | inspection, nullable extraction, fallback, map/chaining, unwrap, `or_throw` | [Result](../02-handbook/15-errors/01-result.md) |
| `Throwable` | immutable identity | `message()`, `code()`, `cause()`, `suppressed()`, `stack_trace()` | [Exceptions](../02-handbook/15-errors/04-exceptions.md) |
| `Resource<E>` | `is_closed()` | `close()`; type-specific transfer/duplicate operations | [Resources](../02-handbook/16-resources/README.md) |
| `Environment` / `Env` | — | static get/default/required/secret/contains/list operations | [`std.environment`](../04-standard-library/18-std-environment.md) |
| `Weak<T>` | `is_alive` | static `from()`, `upgrade()` | [Safe References](../02-handbook/17-memory-and-safety/04-safe-references.md) |
| `Pointer<T>` | `is_null` | `read()`, `write()`, `offset()`, `offset_bytes()`, `cast<T>()`, volatile access | [Pointers](../02-handbook/17-memory-and-safety/05-pointers.md) |
| native slices | `length`, `is_empty` | bounds-checked indexing/iteration; mutable writes for `NativeSliceMut<T>` | [Pointers](../02-handbook/17-memory-and-safety/05-pointers.md) |
| `Task<T>` | completion state through API | `cancel()`, aggregation through `Task.all/first/settled`, await | [Tasks](../02-handbook/18-concurrency/02-tasks.md) |
| `Channel<T>` | `is_closed`, `capacity`, `length` | `send()`, `receive()`, `try_send()`, `try_receive()`, `close()` | [Channels](../02-handbook/18-concurrency/07-channels.md) |
| synchronization | type-specific | scoped lock access, permits/barriers/once, supported atomic operations | [`std.sync`](../04-standard-library/10-std-sync.md) |
| tuples | `length` | constant `[N]` indexing, destructuring, derived capabilities | [Tuples](../02-handbook/10-data-types/00-tuples.md) |
| domain values | `type` plus declared attributes | declared methods and satisfied contract members | [Data Types](../02-handbook/10-data-types/README.md) |

Members such as equality, hashing, ordering, iteration, indexing, arithmetic, and cloning are capability-gated. Their presence must not be inferred merely from `Object`.

---

**Previous:** [← Feature Status](12-feature-status.md) · **Next:** [ Temporal Reference](14-temporal-reference.md)
