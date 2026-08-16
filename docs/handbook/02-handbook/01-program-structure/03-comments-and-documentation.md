# Comments and Documentation

Comments explain intent that names and structure cannot express. Zirk supports line and block comments:

```zirk
// Explain why this retry limit exists.
inmut MAX_RETRIES = 3;

/* Explain a constraint that spans
   more than one line. */
```

Documentation uses documentation-form multiline comments associated with declarations. Public API documentation should describe contracts: valid inputs, returned values, expected failures, permissions, cancellation, blocking, and resource ownership. Repeating a function's name is not useful documentation.

Comments do not disable type or permission rules. Avoid using commented-out source as version history; source control already preserves it.

The public Syntax API retains documentation metadata for tools and `zirk doc`. Generated documentation must not expose private implementation declarations merely because they carry comments.

---

**Previous:** [← Lexical Rules](02-lexical-rules.md) · **Next:** [ Statements and Semicolons](04-statements-and-semicolons.md)
