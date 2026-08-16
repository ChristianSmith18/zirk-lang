# `share`

`share` publishes a declaration from its file.

```zirk
share class User {}
```

Publication makes a name importable; it does not bypass member visibility or package API rules. Treat every shared declaration as a compatibility commitment and keep implementation helpers private.

---

**Previous:** [← File Modules](01-file-modules.md) · **Next:** [ import](03-import.md)
