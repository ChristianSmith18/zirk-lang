# Interface, Trait, or Class?

- Use an **interface** for a behavior contract with no required reusable implementation.
- Use a **trait** for a contract plus composable behavior.
- Use an **abstract class** for a nominal set of required attributes and
  abstract functions, adopted through `implements` without state or bodies.
- Use a **class** for identity, state, construction, lifecycle, or one inheritance lineage.

An interface answers “what can this value do?” A trait may also answer “what common implementation follows?” A class answers “what kind of identity-bearing object is this?”

Choose the smallest abstraction that communicates the contract; premature hierarchy makes compatibility harder.

One design can use all three deliberately:

```zirk
interface Repository<T> {
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

    audit_name(): String {
        return "users";
    }

    find(id: UInt64): Result<User, RepositoryError> {
        this.audit("find {id}");
        return this.connection.find_user(id);
    }
}
```

`Repository<T>` is the substitutable capability, `Auditable` contributes
shared implementation, and `UserRepository` owns state, identity, construction,
and lifecycle.

---

**Previous:** [← Default Implementations](04-default-implementations.md) · **Next:** [ Errors](../15-errors/README.md)
