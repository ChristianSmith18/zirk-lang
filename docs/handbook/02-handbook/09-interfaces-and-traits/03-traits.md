# Traits

Traits define behavior contracts and may include reusable implementations. A class can combine multiple traits while extending at most one class.

Traits suit cross-cutting capabilities such as display, cloning, iteration, or serialization. They should not hide mutable shared state or unresolved method conflicts.

When combined traits provide incompatible members, the type must resolve the conflict explicitly; declaration order must not silently choose semantics.

```zirk
trait Loggable {
    fn log_prefix(): String;

    fn log(message: String): Void {
        stdout.println("[{this.log_prefix()}] {message}");
    }
}

class UserService implements Loggable {
    fn log_prefix(): String {
        return "users";
    }
}

UserService().log("created");
```

The trait supplies reusable behavior while the class provides the requirement
that behavior needs.

---

**Previous:** [← Implementing Contracts](02-implementing-contracts.md) · **Next:** [ Default Implementations](04-default-implementations.md)
