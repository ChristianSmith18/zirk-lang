# Formatter

The formatter is canonical and idempotent. It normalizes source layout without
changing tokens, declaration order, or program meaning, and has no fragmented
style-configuration surface.

`zirk format <target>...` accepts files and directories (directories are
searched recursively for `.zrk` files); a bare file name may omit `.zrk`.
`--check` reports differences and exits nonzero without writing, while `--diff`
prints proposed changes without writing. Invalid input reports diagnostics and
is never partially rewritten. Formatting the same bytes twice produces
identical bytes.

Canonical output uses four-space indentation and a 100-column width. A
comma-separated group that exceeds that width breaks every element onto its own
indented line; a fitting group remains inline. It removes trailing whitespace,
normalizes tabs, and preserves `'''` literal layout. Leading comments follow
the next construct, trailing comments stay on their line after one space, and
comments in a broken list remain on their own list-indented line.

Use `// zirk-compiler: fmt off` and `// zirk-compiler: fmt on` to retain a
region byte-for-byte. Directives do not nest; an unclosed `fmt off` continues
to EOF. The formatter cannot reorder effects, change imports, rewrite explicit
types, or expand source sugar such as promoted constructor parameters.

```bash
zirk format src/ examples/main
zirk format --check src/
zirk format --diff examples/main.zrk
```

The same engine is used by the CLI, language server, and browser package.

---

**Previous:** [← CLI](08-cli.md) · **Next:** [ Linter](10-linter.md)
