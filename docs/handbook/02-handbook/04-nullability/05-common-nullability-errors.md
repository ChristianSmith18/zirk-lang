# Common Nullability Errors

## Access without proof

```zirk
mut user: User? = find_user();
stdout.println(user.name); // invalid
```

Narrow `user`, use safe access, or provide a fallback. The compiler should point to the nullable receiver rather than reporting a missing member.

## Assigning a nullable value to a required binding

```zirk
inmut title: String = configured_title; // configured_title is String?
```

Handle `null` first. An implicit assertion would turn a visible type obligation into a runtime surprise.

## Treating nullable Boolean as a condition

```zirk
mut enabled: Boolean? = null;
if enabled { /* invalid */ }
```

Decide what `null` means: `if enabled ?? false`, or branch among all three states.

## Confusing absence with failure

Use `T?` when “not present” is an ordinary value. Use `Result<T, E>` when the caller needs an explanation of why an operation failed.

---

**Previous:** [← Flow Analysis](04-flow-analysis.md) · **Next:** [ Operators and Expressions](../05-operators-and-expressions/README.md)
