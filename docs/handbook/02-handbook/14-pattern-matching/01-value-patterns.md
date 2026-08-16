# Value Patterns

A value pattern selects a branch when the subject equals a literal or constant according to the relevant equality contract.

```zirk
match exit_code {
    0 { stdout.println("success"); }
    1 { stdout.println("general failure"); }
    _ { stdout.println("other failure"); }
}
```

The wildcard `_` matches without binding. Put specific patterns before a catch-all; an unreachable later branch should produce a warning or diagnostic.

Patterns must be compatible with the matched type. Comparing an integer subject with a string pattern is a type error, not a branch that simply never runs.

Regex literals are value patterns for strings. They use the `re'pattern'` form:

```zirk
match input {
    re'^[0-9]+$' => stdout.println("integer text");
    re'^[a-zA-Z]+$' => stdout.println("letters");
    _ => stdout.println("mixed input");
}
```

A regex branch is selected when the complete pattern contract matches the
subject. Place a more specific regex before a broader one; the compiler cannot
always prove that two arbitrary regular expressions overlap.

---

**Previous:** [← Pattern Matching](README.md) · **Next:** [ Multiple Patterns](02-multiple-patterns.md)
