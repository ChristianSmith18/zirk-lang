# Operators and Precedence

Operators are grouped below from strongest to weakest binding. Parentheses always make intent explicit. Short-circuiting operators evaluate their right operand only when required.

| Level | Operators | Meaning/availability |
|---:|---|---|
| 1 | `()`, `[]`, `.`, `?.` | grouping/call, indexing, member and safe access |
| 2 | postfix `++`, `--` | mutable numeric place only |
| 3 | prefix `!`, `+`, `-`, `~`, `++`, `--`, unsafe `*` | Boolean not; numeric sign; integer complement; mutable numeric place; raw pointer dereference |
| 4 | `**` | numeric power; right-associative |
| 5 | `*`, `/`, `%` | numeric; String repetition for `String * Integer` and reverse form |
| 6 | `+`, `-` | numeric; `String + String`; temporal combinations listed below |
| 7 | `<<`, `>>` | fixed-width integers |
| 8 | `&` | integers or explicitly contracted domain types |
| 9 | `^` | integers or explicitly contracted domain types |
| 10 | `\|` | integers or explicitly contracted domain types |
| 11 | `<`, `<=`, `>`, `>=` | ordered compatible operands |
| 12 | `==`, `!=`, `is` | structural/content equality; identity only for references |
| 13 | `&&` | short-circuit Boolean conjunction |
| 14 | `\|\|` | short-circuit Boolean disjunction |
| 15 | `??` | null coalescing |
| 16 | `?:` | conditional expression; right-associative |
| 17 | `\|>` | pipeline |
| 18 | `=`, `+=`, `-=`, `*=`, `/=`, `%=` and bitwise/shift compounds | assignment; right-associative and requires an assignable mutable place |

## Native result rules

| Operands | Supported operators | Result/notes |
|---|---|---|
| integer + integer | arithmetic, comparison, bitwise, shifts | promoted compatible integer; `/` truncates toward zero; `%` follows dividend sign |
| integer + Float | arithmetic, comparison | compatible Float width |
| Float + Float | arithmetic, comparison | compatible Float; infinities allowed, `NaN`-producing operations fail |
| Boolean | `!`, `&&`, `\|\|`, equality | `Boolean`; no truthiness, order, or arithmetic |
| Char | equality, lexical order | comparison is locale-independent Unicode ordering |
| String | equality/order, `+`, integer `*` | content comparison, concatenation, checked repetition |
| Date/DateTime/ZonedDateTime + Period | `+`, `-` | same temporal type; calendar arithmetic |
| Time/DateTime/Instant/ZonedDateTime + Duration | `+`, `-` | same type, except `Time` returns `TimeShift` |
| like temporal values | subtraction where defined | `Duration` or calendar result as documented |
| Duration | unary sign, arithmetic, scalar `*`/`/`, ratio `/`, `%`, order | exact signed duration or scalar result |
| Period | `+`, `-`, integer scale, equality | no context-free ordering or total duration |
| nullable | `?.`, `??`, equality with `Null` | narrowed or fallback result |
| reference | `is` | identity; structural `==` only with equality capability |
| `Pointer<T>` + integer | `+`, `-`, unsafe prefix `*` | element-measured address offset or raw dereference; requires `unsafe` |

`Pointer<T>.read()`/`write()` are the preferred auditable equivalents of raw
dereference syntax. Byte offsets use `Pointer<Byte>` or `offset_bytes`; pointer
representation casts use `cast<T>()`. None of these operations bypasses
transaction, `commit`, lifetime, or permission rules.

Compound assignment is equivalent in type behavior to reading the left value, applying the operator, validating the result, and writing it back once. It never relaxes conversion or mutability rules.

Native arithmetic reports controlled errors for division by zero, checked overflow, invalid shifts, invalid Float operations, invalid repetition counts, and allocation overflow. See [Native Operators](../02-handbook/03-everyday-types/01e-native-operators.md).

---

**Previous:** [← Keywords](01-keywords.md) · **Next:** [ Built-in Types](03-built-in-types.md)
