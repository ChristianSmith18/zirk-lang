# `while`

`while` repeats a block while a Boolean condition remains true.

```zirk
mut attempts = 0;
while attempts < 3 {
    attempts += 1;
    try_connect();
}
```

The condition is checked before each iteration, so the body may run zero times. Values needed afterward must satisfy definite-initialization rules on both the entered and skipped paths.

Prefer `for ... in` when consuming an iterable; use `while` when progress depends on changing state or an external condition.

When the body must run before the first condition check, use `do ... while`:

```zirk
do {
    read_next_page();
} while has_more_pages;
```

The trailing semicolon terminates the complete statement. A `do ... while`
body always executes at least once; a `while` body may execute zero times.

---

**Previous:** [← for ... in](06-for-in.md) · **Next:** [ loop](08-loop.md)
