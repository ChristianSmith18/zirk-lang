# `if` and `else`

`if` selects a branch from a `Boolean` condition. Braces are required and there is no numeric or object truthiness.

```zirk
if response.is_success {
    store(response.value);
} else if response.is_retryable {
    schedule_retry();
} else {
    report_failure();
}
```

Only the selected branch executes. Names introduced inside a branch remain inside its block. A `Boolean?` must be narrowed or given a fallback before use as the condition.

Invalid:

```zirk
if pending_count { process(); }
```

Write `pending_count > 0`; the diagnostic should state that `Int32` is not `Boolean`.

---

**Previous:** [← Control Flow](README.md) · **Next:** [ if Expressions](02-if-expressions.md)
