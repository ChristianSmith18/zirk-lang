# Blocks and Scope

Braces group declarations and expressions and create lexical scope. A name declared inside a block is unavailable after that block ends.

```zirk
if authenticated {
    inmut greeting = "Welcome back";
    stdout.println(greeting);
}

// greeting is not visible here.
```

Scopes exist at block, function, file, and module levels. Function parameters and locals belong to the function. A file declaration remains private unless `share` publishes it. Application globals are exceptional: they may appear only in the `globals` block of `init.zrk`, and each consumer opts in with `use`.

Invalid example:

```zirk
if ready {
    inmut message = "go";
}
stdout.println(message);
```

The compiler should report that `message` is out of scope and point to its declaration. Move the declaration outward only if the value logically belongs to the wider lifetime.

---

**Previous:** [← Statements and Semicolons](04-statements-and-semicolons.md) · **Next:** [ Naming Conventions](06-naming-conventions.md)
