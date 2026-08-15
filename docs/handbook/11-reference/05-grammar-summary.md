# Grammar Summary

A source file contains declarations and imports. Blocks use braces; parser semicolons may be omitted when unambiguous, while formatting writes them. Declarations include bindings, functions, object/data types, contracts, implementations, decorators, and published declarations.

Expressions include literals (including `re'pattern'` regex literals), names,
calls, member/index access, lambdas with optional leading `fn`, unary, binary and
conditional operators, `if`, and `match`. Statements include
expression/declaration forms, braced or single-statement `if`, traditional
`for`, `for ... in`, `while`, `do ... while`, control transfer, `try`, and unsafe
or concurrent constructs.

Ranges use `start..end` or `start..=end`, with `.step(distance)` and
`.reverse()`. Slices use `[start:end:step]`. Multiple patterns use comma before
one shared `=>` body. Traditional enums may map cases with `->`; algebraic enum
patterns may destructure nested records. This summary does not replace the
machine-readable grammar or its complete precedence and recovery rules.

---

**Previous:** [← Literals](./04-literals.md) · **Next:** [Attributes and Decorators →](./06-attributes-and-decorators.md)
