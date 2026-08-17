# Grammar Summary

A source file contains declarations and imports. Blocks use braces; parser semicolons may be omitted when unambiguous, while formatting writes them. Declarations include bindings, functions, object/data types, contracts, implementations, decorators, and published declarations.

Expressions include literals (including `re'pattern'` regex literals), names,
calls, member/index access, lambdas with optional leading `fn`, unary, binary and
conditional operators, `if`, and `match`. Statements include
expression/declaration forms, braced or single-statement `if`, traditional
`for`, `for ... in`, `while`, `do ... while`, control transfer, `try`, and unsafe
or concurrent constructs.

Low-level syntax includes `unsafe { ... }`, `unsafe fn`, and irreversible
`commit { ... }` regions inside unsafe. Concurrent syntax includes task blocks
or callable sugar, `task scope`, `await expression timeout duration`,
`cancellation shield`, `parallel`, `parallel for`, `thread`, and `select`.

```zirk
select {
    value = await operation => use(value),
    after 5s => timeout(),
    cancelled => cleanup(),
    default => continue_work(),
}
```

`select` runs one ready branch fairly and leaves losing operations alive.
`commit` is invalid outside unsafe, and reversible unsafe regions cannot await
or spawn concurrent work.

Failure syntax includes `throws T | U`, `throw expression`, exact `throw;`
inside catch, `try`, guard-free `catch Type(binding)` patterns, and `finally`.
Resources use `match acquisition with binding`, including grouped acquisition.
Manifests use library `requires`, application `permissions`, and operation
`during: build | runtime | both`.

Ranges use `start..end` or `start..=end`, with `.step(distance)` and
`.reverse()`. Slices use `[start:end:step]`. Multiple patterns use comma before
one shared `=>` body. Traditional enums may map cases with `->`; algebraic enum
patterns may destructure nested records. This summary does not replace the
machine-readable grammar or its complete precedence and recovery rules.

Type expressions include names, generic application (`List<String>`), fixed
arrays (`Float64[4]`), tuples (`Tuple(String, Int)`), unions (`A | B`), nullable
shorthand (`T?`), and callable types (`Function(P...) => R`, preferably
`Fn(P...) => R`). Conversion uses type construction syntax. Concrete classes
use `extends`; interfaces, traits, and state-free abstract requirement classes
use `implements`. Postfix assignment targets must be places; indexed or sliced
assignment obeys permissions and equal-width replacement rules.

Tuple values use `(a, b)` and constant `result[n]` access. Match has no guards,
is exhaustive over closed domains in both forms, and unpacks algebraic enums
only inside match. Slice components may be omitted; explicit bounds remain
strict and slice results are independent deep copies.

---

**Previous:** [← Literals](04-literals.md) · **Next:** [ Attributes and Decorators](06-attributes-and-decorators.md)
