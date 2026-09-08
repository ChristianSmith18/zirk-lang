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

Valid: `_ = operation()` explicitly discards a `Result`. Invalid:
`operation();` when its value is `Result`. Valid: `catch NetworkError(error)`.
Invalid: historical `catch<NetworkError> error`. Valid: a library requests and
an application grants one scoped operation. Invalid: treating an `init.zrk`
edit as developer approval or reusing approval after moving the project.

Valid: the explicit Decimal context reaches the contained division.

```zirk
inmut ratio = Decimal(3 / 4); // 0.75
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

Invalid: `await`, task/thread creation, socket/file/process effects, or an
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

Valid structured aggregation and selection:

```zirk
mut outcomes = await Task.settled(tasks);

select {
    message = await messages.receive() => process(message),
    after 5s => report_timeout(),
}
```

Invalid: assuming `Task.settled` cancels siblings, assuming `select` cancels
losing operations, detaching a task without an application supervisor, holding
an ordinary mutex across `await`, or sharing a writable `List<T>` between tasks
without transfer or synchronization.

---

**Previous:** [← Language Feature Matrix](02-language-feature-matrix.md) · **Next:** [ Differences from TypeScript](04-differences-from-typescript.md)
