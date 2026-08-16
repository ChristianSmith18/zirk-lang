# Multiple Patterns

Several alternatives can share behavior when the pattern grammar groups them into one branch.

```zirk
match status_code {
    200, 201, 204 => handle_success();
    400, 404 => handle_client_error();
    _ => handle_other();
}
```

Use commas—not `|`—between alternative patterns. A long group may span lines:

```zirk
match status_code {
    200,
    201,
    204 => {
        record_success(status_code);
        handle_success();
    }

    400,
    404 => handle_client_error();

    _ => handle_other();
}
```

All alternatives in a branch must establish compatible bindings. A name cannot exist only for one alternative and then be used by the shared body.

The arrow belongs after the final alternative and introduces the one shared
body. Commas inside a constructor pattern still separate its payload fields;
the surrounding grammar makes the two uses unambiguous.

---

**Previous:** [← Value Patterns](01-value-patterns.md) · **Next:** [ Type Patterns](03-type-patterns.md)
