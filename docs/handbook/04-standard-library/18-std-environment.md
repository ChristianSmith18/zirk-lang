# `std.environment`

`std.environment` provides permission-checked, read-only access to the current
process environment. `Environment` is the full contract name and `Env` is its
exact preferred alias. They expose the same static operations.

Environment and network permissions in a package's `.zkinit` declare
requirements; they do not grant access. `zirk permissions approve` resolves
the application's locked dependency graph and asks for explicit consent to
the union of application and package requirements. The approval is bound to
the dependency manifests and lockfiles, so changing a transitive requirement
requires approval again.

Name matching follows the host platform: exact and case-sensitive on Unix,
ordinal and case-insensitive on Windows. Permission checks use the same rule as
host lookups, including for `contains` and `list_names`.

Environment variables are external mutable state, not ordinary globals. Every
read can fail, each name is checked against the effective project grant, and
secret values remain redacted.

> **Implementation status:** this chapter defines the target Zirk 1.x contract.
> Initial Zirk intentionally has no API for mutating the current process
> environment.
>
> The native preview provides generic `Env.get<T>` through the static
> `EnvironmentParsable<T>` contract. `Env.get_int32` and `Env.get_float64`
> remain available as explicit convenience methods. The implemented
> environment subset passes full-workspace native CI on macOS ARM64, Linux
> x86_64, Linux ARM64, and Windows x86_64 (run 39359107965, compiler revision
> b28daa7); debugger, snapshot, test-report, and additional reveal sinks remain
> staged.

## Import and basic access

```zirk
import { Env } from std.environment;
import { stdout } from std.io;

inmut mode = Env.get("APP_MODE"); // String?; Error becomes null
stdout.println(mode ?? "development");
```

The retrieval method always returns a typed Result. Direct matching preserves
the error; ordinary value use projects it to a nullable value:

```zirk
inmut result: Result<String, EnvironmentError> = Env.get("APP_MODE");
match result {
    Ok(mode) => stdout.println(mode);
    Error(error) => stderr.println(error);
}
```

| API | Type |
| --- | --- |
| `Env.get(name)` | `Result<String, EnvironmentError>` |
| `Env.get<T>(name)` | `Result<T, EnvironmentError>`; `T` defaults to `String` |
| `Env.require(name)` | `Result<String, EnvironmentError>` |
| `Env.contains(name)` | `Result<Boolean, EnvironmentError>` |
| `Env.list_names()` | `Result<List<String>, EnvironmentError>` |
| `Env.get_secret(name)` | `Result<SecretString, SecretError>` |

`get` and `require` both demand a present value and currently return the same
`EnvironmentError` outcomes; `require` is an explicit spelling for mandatory
application settings. `Env.get_or_null` and `Env.get_or` were removed because
ordinary projection and `??` express those value-use policies. Remember that
`??` uses the fallback for every `Error`, not only authorized absence.

```zirk
match Env.get("LOG_LEVEL") {
    Ok(level) => configure_logging(level);
    Error(error) => report_configuration_error(error);
}

match Env.get<Int32>("WORKER_COUNT") {
    Ok(count) => configure_workers(count);
    Error(error) => report_configuration_error(error);
}

mut worker_count = Env.get<Int32>("WORKER_COUNT") ?? 4;
stdout.println(worker_count);
```

## Typed values

Generic reads use the ordinary parsing contract of the requested type:

```zirk
mut port = Env.get<Int32>("PORT");
mut timeout = Env.get<Float64>("TIMEOUT_SECONDS");
mut label = Env.get("APP_LABEL"); // T defaults to String
mut workers: Int32? = Env.get<Int32>("WORKERS");
mut workers_with_default = Env.get<Int32>("WORKERS") ?? 4;
```

The target result distinguishes:

- `EnvironmentError.Missing` when a required name is authorized but absent;
- `EnvironmentError.Parse` when a present value is not valid for `T`;
- `EnvironmentError.PermissionDenied` when the project lacks authority;
- a platform/encoding error when the host value cannot be represented safely.

Supported `T` satisfies `EnvironmentParsable<T>`, whose static
`parse(String)` returns `Result<T, String>`. This is compile-time constrained;
`Env.get<T>` does not use runtime reflection or guess constructors. User types
can implement the contract with explicit parsing:

```zirk
import { EnvironmentParsable, Env } from std.environment;

class Port implements EnvironmentParsable<Port> {
    value: String;
    construct(value: String) { this.value = value; }
    static parse(text: String): Result<Port, String> {
        if (text == "") { return Error("port is empty"); }
        return Ok(Port(text));
    }
}

mut port = Env.get<Port>("PORT");
```

A parser failure maps to `EnvironmentError.Parse`. Direct matching retains
`Parse`, permission denial, decoding, and platform errors. Projection maps any
of these errors to `null`; a following `??` therefore defaults on all errors.
Built-in string, numeric, decimal, and temporal parsers use compiler-owned
intrinsics and satisfy the same constraint. The named `get_int32` and
`get_float64` methods remain explicit convenience wrappers.

Named shorthand follows the language-wide rule:

```zirk
mut name = "PORT";
mut port = Env.get<Int32>(name:);
```

`name:` means `name: name`; it does not perform positional name matching.

## Permissions and non-disclosure

Applications grant exact names or reviewed name patterns in `.zkinit`.
Libraries declare requirements but cannot grant themselves access. Runtime code
never prompts or edits the manifest.

After declaring permissions, approve the exact request from the project root:

```sh
zirk permissions approve .
```

The command prints the scopes and saves consent only after you type `approve`.
`zirk run .` uses that signed approval only while the current manifest still
matches. A missing, changed, or invalid approval grants no access.
Revoke the saved approval with `zirk permissions revoke .`.

An unauthorized read returns `EnvironmentError.PermissionDenied` without
revealing whether the requested variable exists. `contains` follows the same
rule, so it cannot be used as an existence side channel.

`list_names()` returns only names visible under the effective grant, in
lexicographic order for stable results across platforms. Discovering all
environment names requires an explicit broad permission and the normal
high-risk developer approval. It never returns values; temporary process-local
copies used by `contains()` and `list_names()` are zeroized after inspection.
This does not erase the operating system's environment storage.

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

`SecretString` exposes no raw-value accessor. Direct formatting,
`to_string()`, interpolation, collection rendering (including lists and map values), and `Throwable.message()` redact it. Two explicit reveal sinks accept it directly:
`HTTP.request_with_secret_header(method, url, body, headers, name, secret)`.
It accepts the secret directly, requires an HTTPS URL, rejects a duplicate
header name, and sends only after the HTTP runtime validates the exact network
origin grant. It does not follow redirects. The secret is never returned to
Zirk as an ordinary `String`.

For proxy authentication, pass the value to
`HTTPProxy.create_authenticated(proxy_url, credentials)` and configure that
proxy on an `HTTPClient`. The value must contain `username:password`; the
runtime rejects an empty username, username colons, and control bytes before
networking. Credentials can be sent to loopback HTTP proxies or HTTPS proxies;
remote cleartext HTTP proxies fail closed. For HTTPS proxies, Zirk validates
the proxy TLS identity before sending credentials, and never forwards proxy
authorization through the destination tunnel. The client separately checks
the proxy and destination grants. Formatting the proxy redacts its credentials.

The current formatter, collection rendering, interpreter execution trace, and
fatal runtime diagnostics redact secret payloads. Runtime end-to-end coverage
checks direct formatting, nested list and map values, interpolation,
secret-bearing throwables, and the JSON diagnostic output. The dedicated
[maintainer checklist](../../development/SECRET_REDACTION_CHECKLIST.md) is a
review gate for any new output sink. Debugger views, heap snapshots, test
reports, and structured tool-output serializers are not implemented or remain
unverified; none should be described as redaction-safe until it has
sink-specific executable coverage.

`SecretString` prevents accidental disclosure through ordinary language sinks; it is not a locked or zeroizing heap allocation. The runtime zeroizes the temporary native copy after transferring the value into the managed heap, but the managed value follows ordinary garbage-collector lifetime and is not promised to be erased when collected.

The wrapper defines no public equality or hashing API and carries no
per-instance permission metadata. Secret-name permission is checked when
`get_secret` reads the value; later reveal remains limited to explicit sink
APIs. Wrapping or copying a value does not grant access to another environment
name or bypass the sink's own permission checks.

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

Environment reads are synchronous process-environment lookups; they do not
perform asynchronous I/O or suspend a cooperative branch. This contract does
not imply that arbitrary filesystem, process, or network operations are
non-blocking. Reads allocate the returned safe value. Implementations may cache
immutable decoded metadata, but must not cache a permission decision beyond its
effective signed policy or silently return a stale secret after authority
changes.

---

**Previous:** [← `std.system`](17-std-system.md) · **Next:** [Standard Library Indexes →](19-standard-library-indexes.md)
