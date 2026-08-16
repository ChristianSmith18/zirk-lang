# No Traditional Overloading

Zirk 1.x does not select among multiple same-name functions by argument types. Use distinct names, generics, or union parameters.

```zirk
fn parse_int(text: String): Result<Int32, ParseError> { /* ... */ }
fn parse_float(text: String): Result<Float64, ParseError> { /* ... */ }
```

This keeps name resolution deterministic and diagnostics direct. A generic function represents one algorithm across types; a union represents one API that intentionally handles alternatives.

Operator overloading is a narrow exception governed by language-defined contracts, not evidence of general overload sets.

Classes also permit multiple `construct` declarations with distinct effective
parameter signatures. Constructor resolution is a construction-specific rule;
ordinary named functions and methods still cannot form overload sets.

---

**Previous:** [← Mutability in Parameters](09-mutability-in-parameters.md) · **Next:** [ Never-Returning Functions](11-never-returning-functions.md)
