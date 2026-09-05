# Regex

A `Regex` is a compiled pattern. Zirk's canonical syntax is the `re'...'`
literal, where the pattern is checked at compile time whenever possible. Invalid
patterns are compile-time errors for literals; dynamic patterns use
`Regex.parse` and return a typed `Result`.

## Literals

```zirk
inmut digit = re'[0-9]+';
inmut identifier = re'[A-Za-z_][A-Za-z0-9_]*';
```

The `re'...'` form produces a compiled `Regex` value. Escapes inside the pattern
follow the regex grammar, not String escape rules.

```zirk
inmut quoted = re'"[^"]*"'; // matches a double-quoted, non-nested string
```

Invalid patterns are rejected where the literal is compiled:

```zirk
inmut broken = re'[a-z'; // error: unclosed character class
```

## Matching the whole text

`matches` returns `true` when the pattern matches the entire input:

```zirk
inmut digit = re'[0-9]+';

digit.matches("2026");  // true
digit.matches("42abc"); // false
digit.matches("");      // false
```

## Finding a substring

`find` returns a `Regex.Match?` describing the first match, or `null` if none.
The match object exposes positional and named capture groups through `group`:

```zirk
inmut date = re'(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})';
inmut found = date.find("Log: 2026-09-05");

match found {
    null => stdout.println("no date"),
    m => stdout.println("{m.group("year")}"),
}
```

## Replacement

`replace` returns a new `String` with all matches replaced:

```zirk
inmut digit = re'[0-9]+';
inmut redacted = digit.replace("Room 42, floor 7", "X");

// redacted == "Room X, floor X"
```

The original string is unchanged. `String` is a managed reference, so `replace`
produces a new instance.

## Regex as a value pattern

A regex literal can be used directly in `match`:

```zirk
match input {
    re'^[0-9]+$' => stdout.println("digits"),
    re'^[a-zA-Z_][a-zA-Z0-9_]*$' => stdout.println("identifier"),
    _ => stdout.println("other"),
}
```

The match is structural: the pattern is tested as a value, not as a control-flow
expression.

## Dynamic patterns

When the pattern is not known at compile time, use `Regex.parse`:

```zirk
inmut pattern = "[a-z]+";
inmut compiled = Regex.parse(pattern);

match compiled {
    Error(e) => stdout.println("invalid pattern: {e}"),
    Ok(r) => r.matches("hello"),
}
```

`Regex.parse` must be used for any pattern that comes from a variable, a file,
or a network response.

## When to choose Regex

- Use `Regex` for pattern-based validation, searching, or transformation.
- Use `String.contains`, `starts_with`, and `ends_with` for literal substrings.
- Prefer `String` slicing and direct comparison when the task does not need a
  pattern.

## API

A compiled pattern. `re'...'` literals are checked at compile time; dynamic
patterns use `Regex.parse` and return a typed `Result`. The standard engine
guarantees linear-time matching (no backreferences or unsafe lookbehind).

> **Delivery caveat:** `re'...'` literals, `matches`, `find` (with positional
> and named groups on `Regex.Match`), `replace`, `Regex.split`, `find_all`,
> match iteration, and `re'...'` inside `match` patterns are delivered.
> `Regex.parse` remains pending.

### Properties

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `pattern` | `String` | Source pattern text | specified — named "pattern" in the member index |

### Methods

| Signature | Returns | Description | Status |
| --- | --- | --- | --- |
| `re'...'` | `Regex` | Compile-time checked literal | implemented |
| `Regex.parse(pattern)` | `Result<Regex, RegexError>` | Dynamic pattern compilation | specified |
| `r.matches(text)` | `Boolean` | Whole-text match | implemented |
| `r.find(text)` | `Regex.Match?` | First match, or `null` | implemented |
| `r.find_all(text)` | `List<Regex.Match>` | All matches | implemented |
| `r.replace(text, replacement)` | `String` | New string with all matches replaced | implemented |
| `r.split(text)` | `List<String>` | Split around matches | implemented |
| `r.to_string()` | `String` | Pattern rendering | specified |

### `Regex.Match`

| Member | Type | Description | Status |
| --- | --- | --- | --- |
| `start` | `Int32` | Grapheme index of match start | implemented |
| `end` | `Int32` | Grapheme index one past match end | implemented |
| `text` | `String` | Matched substring | implemented |
| `m.group(index)` | `String?` | Positional capture | implemented — the current signature returns `String` |
| `m.group(name)` | `String?` | Named capture | implemented — the current signature returns `String` |

### Examples

```zirk
inmut digit = re'[0-9]+';
digit.matches("2026");          // true
digit.replace("Room 42, floor 7", "X");  // "Room X, floor X"

inmut date = re'(?<year>[0-9]{4})-(?<month>[0-9]{2})-(?<day>[0-9]{2})';
match date.find("Log: 2026-09-05") {
    null => stdout.println("no date"),
    m => stdout.println("{m.group("year")}"),
}

match Regex.parse(user_pattern) {
    Error(e) => stdout.println("invalid: {e}"),
    Ok(r) => r.matches(input),
}

// regex literals in match patterns — specified, pending:
match input {
    re'^[0-9]+$' => stdout.println("digits"),
    _ => stdout.println("other"),
}
```

## Implementation status

> `array-list-tuple-duration-regex` delivered `re'...'` literals, `Regex` with
> `matches`, `find` (including `Regex.Match` positional and named groups), and
> `replace`. `Regex.split`, `String.split`, match iteration, and `re'...'` in
> full pattern-matching integration remain pending.

---

**Previous:** [← Duration](11-duration.md) · **Next:** [ Temporal Types](../03a-temporal/README.md)
