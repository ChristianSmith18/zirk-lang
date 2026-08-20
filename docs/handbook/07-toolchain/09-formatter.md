# Formatter

The formatter is canonical and idempotent. It writes semicolons and normalizes whitespace, braces, line breaks, and semantically neutral ordering without a fragmented style configuration surface.

`zirk format` rewrites files; a check mode reports differences and exits
nonzero without writing. Formatting the same bytes twice must produce identical
bytes. Comments, documentation, literals, invalid/recovered source regions, and
generated-source provenance are preserved through a lossless syntax layer.

The formatter may choose line breaks, indentation, spaces, trailing separators,
and brace layout. It cannot reorder effects, change imports with different
visibility, rewrite explicit types, or turn a braced `if` into the
single-statement form. Configuration is deliberately narrow so libraries do not
fragment into incompatible house styles.

```bash
zirk format
zirk format --check
```

> **Implementation status:** canonical formatting is specified; a complete
> lossless syntax layer and production formatter remain tooling work.

---

**Previous:** [← CLI](08-cli.md) · **Next:** [ Linter](10-linter.md)
