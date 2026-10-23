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

Valid: expected failure is matched exhaustively. Each handbook feature page
contains additional focused pairs.

Valid: `operation();` runs a `Result`-returning operation and raises its
failure. Valid: `_ = operation()` deliberately discards either Result outcome.
Invalid: using `?` to propagate `Result`—that operator is excluded from Zirk 1.x.
Valid: `catch NetworkError(error)`.
Invalid: historical `catch<NetworkError> error`. Valid: a library requests and
an application grants one scoped operation. Invalid: treating a `.zkinit`
edit as developer approval or reusing approval after moving the project.

Valid: integer division is mathematical and produces an exact `Decimal`.

```zirk
inmut ratio = 3 / 4; // Decimal 0.75
```

An outer conversion acts only on the completed result: `Int32(3 / 4)` is `0`,
while `Float64(3 / 4)` is binary `0.75`. It never retypes operands internally.

Valid String repetition and explicit value conversion:

```zirk
inmut laugh = "ja" * 3;                   // "jajaja"
inmut label = "items=" + String(4);       // "items=4"
```

Invalid forms include `"ja" * -1`, `"ja" * 1.5`, `"items=" + 4`, and
`String("items=" + 4)`.

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

Valid transactional native mutation:

```zirk
unsafe {
    state.status = "updating";
    mut view = match pointer.as_slice_mut(length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    view[0] = marker;
    match validate(view) {
        Ok(_) => {},
        Error(error) => return Error(error), // restores both writes
    }
}
```

Invalid: suspension or concurrent work, socket/file/process effects, or an
unbounded raw write inside that reversible region. Irreversible effects require
an explicit boundary after validation:

```zirk
unsafe {
    mut packet = match validate_packet(pointer, length) {
        Ok(validated) => validated,
        Error(error) => return Error(error),
    };
    commit {
        socket.send(packet.bytes());
    }
}
```

Valid structured concurrency:

```zirk
fn calculate(): Int32 { return 42; }

concurrent {
    inmut job = spawn calculate();
    inmut result = job.wait();
}
```

Invalid: using the removed `task`, `await`, or `select` constructs; assuming
`parallel.settled` is a language API; detaching a branch without an application
supervisor; or sharing a writable `List<T>` between concurrent branches without
transfer or synchronization.

---

**Previous:** [← Language Feature Matrix](02-language-feature-matrix.md) · **Next:** [ Differences from TypeScript](04-differences-from-typescript.md)
