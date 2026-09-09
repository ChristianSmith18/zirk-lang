# `std.environment`

`std.environment` provides permission-checked, read-only access to the current
process environment. `Environment` is the full contract name and `Env` is its
exact preferred alias. They expose the same static operations.

Environment variables are external mutable state, not ordinary globals. Every
read can fail, each name is checked against the effective project grant, and
secret values remain redacted.

> **Implementation status:** this chapter defines the target Zirk 1.x contract.
> Initial Zirk intentionally has no API for mutating the current process
> environment.

## Import and basic access

```zirk
import { Env } from std.environment;

match Env.get("APP_MODE") {
    Ok(mode) => stdout.println(mode);
    Error(error) => stderr.println(error);
}
```

Every public read preserves permission failure:

```zirk
Env.get(name)             // Result<String, EnvironmentError>
Env.get_or_null(name)     // Result<String?, EnvironmentError>
Env.get_or(name, value)   // Result<String, EnvironmentError>
Env.require(name)         // Result<String, EnvironmentError>
Env.contains(name)        // Result<Boolean, EnvironmentError>
Env.list_names()          // Result<List<String>, EnvironmentError>
Env.get_secret(name)      // Result<SecretString, SecretError>
```

`get` and `require` both demand a present value; `require` produces the more
configuration-oriented missing-value diagnostic intended for mandatory
application settings. `get_or_null` returns a nullable value only for an
authorized but absent variable. `get_or` returns its fallback only for that
same absent case. Neither absence form hides permission, decoding, or platform
failure.

```zirk
match Env.get_or("LOG_LEVEL", "info") {
    Ok(level) => configure_logging(level);
    Error(error) => report_configuration_error(error);
}
```

## Typed values

Generic reads use the ordinary parsing contract of the requested type:

```zirk
mut port = Env.get<Int>("PORT");
mut debug = Env.get<Boolean>("DEBUG");
mut endpoint = Env.get<URL>("API_URL");
mut workers = Env.get_or<Int>("WORKERS", 4);
```

The result distinguishes:

- `EnvironmentMissingError` when a required name is authorized but absent;
- `EnvironmentParseError` when a present value is not valid for `T`;
- `EnvironmentPermissionError` when the project lacks authority;
- a platform/encoding error when the host value cannot be represented safely.

Supported `T` implements the public environment-value parsing contract. This is
compile-time constrained; `Env.get<T>` does not use runtime reflection or
guess constructors. User-defined configuration types can implement the same
contract with explicit parsing and typed failure.

Named shorthand follows the language-wide rule:

```zirk
mut name = "PORT";
mut port = Env.get<Int>(name:);
```

`name:` means `name: name`; it does not perform positional name matching.

## Permissions and non-disclosure

Applications grant exact names or reviewed name patterns in `.zkinit`.
Libraries declare requirements but cannot grant themselves access. Runtime code
never prompts or edits the manifest.

An unauthorized read returns `EnvironmentPermissionError` without revealing
whether the requested variable exists. `contains` follows the same rule, so it
cannot be used as an existence side channel.

`list_names()` returns only names visible under the effective grant. Discovering
all environment names requires an explicit broad permission and the normal
high-risk developer approval. It never returns values.

Permission approvals remain bound to the project location, requester versions,
dependency paths, and exact scope as described in the permissions model. A
repository edit alone cannot authorize a new environment name.

## Secrets

`Env.get_secret` requires secret-specific authority and returns `SecretString`:

```zirk
match Env.get_secret("DATABASE_PASSWORD") {
    Ok(password) => connect_database(password:);
    Error(error) => report_secret_error(error);
}
```

`SecretString` has no general `to_string()` or ordinary interpolation path.
Formatting, logs, diagnostics, stack traces, debugger views, snapshots, test
reports, and structured tool output show a redacted marker. Deliberate reveal is
limited to an authorized sink boundary, such as a credential field or explicitly
granted child-process environment entry. The sink receives the secret without
turning it into an accidentally printable ordinary `String`.

Equality and hashing operations that could expose a secret through timing or
collections are restricted to their documented secret-safe contracts. Cloning
does not remove redaction or authority metadata.

## No global mutation

Zirk 1.x does not provide `Env.set`, `Env.remove`, or a mutable environment map.
Process-wide environment mutation is unsafe in concurrent programs, leaks
state between libraries and tests, and can race native code.

Configure a child without changing the parent:

```zirk
mut operation = Process.command("worker")
    .env("APP_MODE", "production")
    .run();
```

Child environment inheritance and secret transfer remain governed by
`std.process` permissions. Tests that need alternative variables use an isolated
runner-provided environment rather than mutating the real process environment.

## Errors and portability

Environment names and values follow the host platform's constraints but enter
Zirk only through validated strings. Invalid names, embedded NUL, unavailable
encoding, and platform lookup failures are typed errors. Case sensitivity is
platform-defined and exposed consistently by the operation; portable programs
must not declare two required names that differ only by case.

Environment reads are non-blocking in the Zirk task model and allocate the
returned safe value. Implementations may cache immutable decoded metadata, but
must not cache a permission decision beyond its effective signed policy or
silently return a stale secret after authority changes.

---

**Previous:** [← `std.system`](17-std-system.md) · **Next:** [Standard Library Indexes →](19-standard-library-indexes.md)
