# `std.environment`

`Environment` and its exact preferred alias `Env` expose static, permission-
checked process-environment access:

```zirk
Env.get(name): Result<String,EnvironmentError>
Env.get_or_null(name): String?
Env.get_or(name, fallback): String
Env.require(name): Result<String,EnvironmentError>
Env.get_secret(name): Result<SecretString,SecretError>
Env.contains(name): Boolean
Env.list_names(): Result<List<String>,EnvironmentError>
```

Each name needs an effective environment or secret grant. `list_names` requires
broad read authority. `SecretString` redacts from formatting, diagnostics,
logs, stack traces and debugger display; deliberate reveal is limited to an
authorized boundary. Initial Zirk has no global environment mutation API.

A missing variable is the method's documented expected outcome. An
unauthorized name returns `Error(PermissionDeniedError)` without revealing
whether the variable exists. Deployed programs never prompt for authority.

---

**Previous:** [← `std.system`](17-std-system.md) · **Next:** [Native and Low-Level →](../05-native-and-low-level/README.md)
