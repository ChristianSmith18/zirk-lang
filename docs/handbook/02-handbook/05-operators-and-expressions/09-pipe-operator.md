# Pipe Operator

The pipe operator expresses a left-to-right transformation pipeline when the target callable contract supports it.

```zirk
inmut result = values
    |> filter(is_valid)
    |> map(normalize)
    |> reduce(combine);
```

Pipelines make data flow visible and pair naturally with iterator operations. They do not bypass parameter types, error handling, permission checks, or eager/lazy semantics of the called operation.

Use ordinary calls when argument placement would be surprising or when a pipeline hides important branching.

> **Source note:** The historical inventory describes the pipe operator. Its exact grammar must be confirmed by the final grammar before compiler-tested examples are classified as executable.

---

**Previous:** [← Compound Assignment](08-compound-assignment.md) · **Next:** [ Ranges](10-ranges.md)
