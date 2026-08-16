# Statements and Semicolons

Zirk permits a semicolon to be omitted when parsing remains unambiguous, but the official formatter writes semicolons. Treat formatted output as the canonical repository form.

```zirk
inmut name = "Ada";
stdout.println(name);
```

Braced constructs do not acquire an extra semantic terminator simply because they span lines. An expression assigned to a binding does:

```zirk
inmut label = if enabled {
    "enabled"
} else {
    "disabled"
};
```

This is valid because `if` can produce the value of the selected branch. For a
short two-value choice, prefer the ternary operator:

```zirk
inmut label = enabled ? "enabled" : "disabled";
```

Use the `if` expression when branches need multiple statements, local names, or
comments. In either form, the binding statement ends in `;`.

Newlines are not general statement syntax. Code that becomes ambiguous without a semicolon should be diagnosed rather than interpreted using fragile formatting heuristics.

Run `zirk format` instead of debating local semicolon style. Formatter idempotence means a second run produces no change.

---

**Previous:** [← Comments and Documentation](03-comments-and-documentation.md) · **Next:** [ Blocks and Scope](05-blocks-and-scope.md)
