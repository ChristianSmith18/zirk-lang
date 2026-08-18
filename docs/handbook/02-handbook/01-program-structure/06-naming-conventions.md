# Naming Conventions

Names communicate category before a reader follows a definition. Zirk's official conventions are:

- `snake_case` for variables, functions, methods, parameters, and filenames;
- `UpperCamelCase` for classes, interfaces, traits, records, and enums;
- `UPPER_SNAKE_CASE` for constants and application globals.

```zirk
record BuildResult {
    output_path: String;
}

inmut DEFAULT_TARGET = "host";
fn resolve_target(requested_target: String?): String { /* ... */ }
```

Case is not semantic mutability: an uppercase name does not become immutable automatically. The declaration keyword and type contract decide that.

Import and destructuring aliases use `Original -> Alias`; named arguments use
`name: value`, and `name:` abbreviates `name: name` when a same-named variable
is visible. Choose aliases that remove ambiguity rather than conceal the source
API.

The formatter normalizes layout but does not rename public APIs. Linters may warn about convention violations without changing program semantics.

---

**Previous:** [← Blocks and Scope](05-blocks-and-scope.md) · **Next:** [ Bindings and Values](../02-bindings-and-values/README.md)
