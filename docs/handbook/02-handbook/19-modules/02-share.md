# `share`

`share` publishes a declaration from its file.

```zirk
share class User { }
```

Publication makes a name importable; it does not bypass member visibility or package API rules. Treat every shared declaration as a compatibility commitment and keep implementation helpers private.

A package root can publish a module implemented by another file without
exposing that physical path:

```zirk
package acme.cli;

share mod terminal from "./terminal";
```

Only `share`d declarations inside `terminal.zrk` are public through
`terminal`; helpers remain private. `mod` is a namespace, not a second kind of
package.

---

**Previous:** [← File Modules](01-file-modules.md) · **Next:** [ import](03-import.md)
