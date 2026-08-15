# Valid and Invalid Examples

Valid: nullable absence is explicit.

```zirk
mut name: String? = null;
inmut label = name ?? "Unknown";
```

Invalid: `null` cannot inhabit `String`.

```zirk
inmut name: String = null;
```

Valid: expected failure is matched exhaustively. Invalid: using `?` to propagate `Result`—that operator is excluded from Zirk 1.x. Each handbook feature page contains additional focused pairs.

---

**Previous:** [← Language Feature Matrix](./02-language-feature-matrix.md) · **Next:** [Differences from TypeScript →](./04-differences-from-typescript.md)
