# Linter

The linter shares parser, names, and types with the compiler. Default rules remain fast; expensive analysis is explicit. Fixes must be safe and reviewable, and warnings cannot redefine valid source.

Default rules cover unused/unreachable declarations, redundant casts,
confusing code shapes, ignored operation results, excessive permission scope,
and APIs with safer documented alternatives. Type- or flow-dependent rules use
the same resolved program as `check`; they do not reimplement a second language.

```bash
zirk lint
zirk lint --json
zirk lint --fix
```

A fix carries exact source edits and applies only when the input revision still
matches. Potentially behavior-changing suggestions remain diagnostics, not
automatic fixes. Suppression is narrow, named by stable rule, visible to tools,
and cannot suppress compiler errors or decorator-generated compiler diagnostics.

Expensive whole-project/security rules are opt-in locally and suitable for CI.
`--warnings-as-errors` changes command success, never the meaning or type of a
program.

---

**Previous:** [← Formatter](09-formatter.md) · **Next:** [ Language Server](11-lsp.md)
