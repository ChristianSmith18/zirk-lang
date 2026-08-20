# Build and Publish a Library

Create `slug`, a library with one published pure function and no authority.

```text
project {
    name: "slug";
    version: "1.0.0";
    type: library;
    entry: "src/slug.zrk";
}
```

```zirk
// src/slug.zrk
share fn slugify(source: String): String {
    return source.trim().lowercase().replace(re'[^a-z0-9]+', "-");
}
```

Only `share`d API enters `public.api`; helpers remain private. If a later
version needs filesystem/network/process behavior, declare the narrow operation
under `requires`. The consuming application remains the only authority that can
grant it.

```bash
zirk format --check
zirk lint
zirk test
zirk prepare
zirk package
zirk publish
```

`prepare` checks documentation, license, public API compatibility, target/native
metadata, permission requirements, lock integrity, and reproducibility. Inspect
the generated package: manifest, `public.api`, portable IR, README, LICENSE, and
documentation. It must contain no secret, ambient path, undeclared generated
file, or target object presented as portable implementation.

Publish a new immutable semantic version; never replace bytes for an existing
version. Test source behavior plus a consumer fixture that imports only the
published API. See [Packages](../09-packages/README.md),
[Libraries](../03-projects/04-libraries.md), and
[Public API](../09-packages/02-public-api.md).

## Completion contract

- **Prerequisites:** modules, `share`, library `requires`, testing, and packages.
- **Expected success:** a verified immutable `.zpkg` with one documented public
  function and reproducible metadata.
- **Failure recovery:** preparation blocks incompatible API, missing license,
  secret/path leakage, or undeclared authority before publication.
- **Next:** wrap a target-specific [C library](06-call-a-c-library.md).

---

**Previous:** [← Build an HTTP Service](04-build-an-http-service.md) · **Next:** [ Call a C Library](06-call-a-c-library.md)
