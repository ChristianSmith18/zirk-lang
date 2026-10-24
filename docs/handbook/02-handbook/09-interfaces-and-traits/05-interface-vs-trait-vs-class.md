# Contract, Trait, or Class?

- Use a **contract** for required properties and operation signatures.
- Use a **trait** for reusable behavior and explicit default selection.
- Use an **abstract class** for nominal requirements that participate in class
  dispatch and inheritance.
- Use a **class** for identity, state, construction, lifecycle, or one
  inheritance lineage.
- Use **Object** for open, dynamically extensible object data.

A contract answers “which shape and operations must this value provide?” A trait
answers “which behavior can these types reuse?” A class answers “which
identity-bearing object is this?”

```zirk
contract Repository<T> {
    find(id: UInt64): Result<T, RepositoryError>;
}

trait Auditable {
    audit_name(): String;

    audit(action: String): Void {
        stdout.println("{this.audit_name()}: {action}");
    }
}

class UserRepository implements Repository<User>, Auditable {
    connection: DatabaseConnection;

    audit_name(): String { return "users"; }

    find(id: UInt64): Result<User, RepositoryError> {
        this.audit("find {id}");
        return this.connection.find_user(id);
    }
}
```

`Repository<T>` describes the capability, `Auditable` contributes shared
implementation, and `UserRepository` owns state, identity, construction, and
lifecycle. Choose the smallest abstraction that communicates the requirement.

---

**Previous:** [← Default Implementations](04-default-implementations.md) · **Next:** [Errors](../15-errors/README.md)
