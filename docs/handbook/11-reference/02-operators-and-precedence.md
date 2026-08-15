# Operators and Precedence

Families include postfix/prefix `++` and `--`, unary `!` and `-`, exponentiation `**`, multiplicative `* / %`, additive `+ -`, comparisons `< <= > >=`, equality `== != is`, logical `&& ||`, null-safe `?.` and `??`, conditional `?:`, assignment and compounds including `**=`, casts `as`/prefix form, union `|`, range constructors `..`/`..=`, and pipeline `|>`.

Parenthesize mixed expressions when precedence is not obvious. Operator implementations cannot change precedence or arity. The final grammar must publish the complete precedence table before this page can assign numeric levels.

Alternative patterns in `match` use commas, not the union operator `|`. Regex
literals use `re'pattern'`; their delimiters are lexical syntax rather than an
operator.

---

**Previous:** [← Keywords](./01-keywords.md) · **Next:** [Built-in Types →](./03-built-in-types.md)
