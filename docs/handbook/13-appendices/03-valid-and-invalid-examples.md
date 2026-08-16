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

Valid: the explicit Float context reaches the contained division.

```zirk
inmut ratio = Float(3 / 4); // 0.75
```

Without that context, integer division truncates toward zero. `3 / 4` is `0`, and wrapping the already-computed value inside a function does not retroactively change it.

Valid String repetition and contextual conversion:

```zirk
inmut laugh = "ja" * 3;             // "jajaja"
inmut label = String("items=" + 4); // "items=4"
```

Invalid forms include `"ja" * -1`, `"ja" * 1.5`, and plain `"items=" + 4`.

Valid shared reference mutation and strict isolation:

```zirk
inmut text = String("hola");
text[0] = 'H';               // binding stays fixed; referent changes
inmut::strict frozen = text.clone();
// frozen[0] = 'h';          // Error: strict reachable state
```

Valid temporal distinctions:

```zirk
inmut tomorrow = Date(2026, 8, 15) + Period(days: 1);
inmut deadline = Instant.now() + Duration.minutes(30);
```

Adding a `Period` to an `Instant`, localizing a DST overlap without a policy, or passing a negative Duration to a non-negative wait API is invalid.

---

**Previous:** [← Language Feature Matrix](02-language-feature-matrix.md) · **Next:** [ Differences from TypeScript](04-differences-from-typescript.md)
