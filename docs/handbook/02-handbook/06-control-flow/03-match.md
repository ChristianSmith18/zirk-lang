# `match`

`match` selects a branch by comparing a value with patterns. It handles ordinary values, types, algebraic enum variants, unions, and destructuring.

The simplest form replaces a traditional `switch`. Commas group values that share one branch:

```zirk
match letter {
    "a", "e", "i", "o", "u" => stdout.println("vowel");
    _ => stdout.println("not a vowel");
}
```

Regex literals use `re'pattern'` and participate directly in value matching:

```zirk
match input {
    re'^[0-9]+$' => parse_number(input);
    re'^[a-zA-Z_][a-zA-Z0-9_]*$' => use_identifier(input);
    _ => reject(input);
}
```

Algebraic variants select a case and bind the data carried by that case:

```zirk
match status {
    Ready => start();
    Waiting => show_spinner();
    Failed(error) => report(error);
}
```

Statement form controls execution without returning a value. Expression form returns the selected branch value and must be exhaustive. Patterns may bind data, but those bindings exist only in the branch.

Zirk does not use special `capture` or `yield` syntax to extract a match result; branch expressions provide it directly.

---

**Previous:** [← if Expressions](02-if-expressions.md) · **Next:** [ Exhaustiveness](04-exhaustiveness.md)
