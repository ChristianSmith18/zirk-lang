# `std.process`

`Process.run` waits for a result; `spawn` returns a managed `ChildProcess` resource. Executable and arguments are separate, with explicit streams, environment, directory, timeout, cancellation, exit code, signal, and captured data.

```zirk
mut result = await Process.run("git", ["status"]);
```

No shell runs implicitly. Shell execution is a separate dangerous API. Process permission is required; environment access may require an additional capability.

---

**Previous:** [← std.path](04-std-path.md) · **Next:** [ std.collections](06-std-collections.md)
