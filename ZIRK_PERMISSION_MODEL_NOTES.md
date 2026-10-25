# Zirk permission model — design notes

**Status:** historical discussion checkpoint; the final contract is recorded below and in the linked OpenSpec/handbook sources.

**Date:** 2026-10-24

## Goal

Permissions should alert and protect users when code attempts to access a protected resource, without preventing an application from starting merely because some permission has not yet been granted. A denied operation should fail locally and be handleable by the program.

## Core model

Keep one finite vocabulary of capability requirements. A library declares what kinds of protected operations it may need with `requires`; it does not grant itself access. Requirements propagate transitively through dependencies to the consuming application.

At project load, Zirk builds an internal permission tree that records the application, dependency path, package identity/version, and each declared requirement. This tree is used to explain the origin of a request. It need not be exposed as a separate user-authored permissions file.

Separate these concepts even if `.zkinit` stores both:

- **Requirement:** a package/API may need a capability, such as `network.listen`.
- **Consent/grant:** a user or administrator approved a concrete resource, such as `127.0.0.1:8087`, for this application.

Declaring a requirement is not consent. Installing a dependency is not consent. A library cannot grant itself or another library authority. The final application does not need to copy every transitive library requirement by hand.

## Permission vocabulary and concrete scopes

Use understandable categories with distinct operations and resource scopes. A broad category must not silently grant every operation in that category.

Examples of capability kinds:

- `filesystem.read(paths)` and `filesystem.write(paths)`
- `network.connect(origins)` and `network.listen(addresses)`
- `network.udp.send(destinations)` and `network.udp.listen(addresses)`
- `network.inspect_interfaces`
- `environment.read(names)` and `environment.list_names`
- `secrets.read(names)`
- `process.execute(executables, argument_rules)` and the separately high-risk `process.shell`
- Native/FFI capabilities when supported by concrete APIs

Scopes are precise resources, not new permission categories. Read does not imply write; connect does not imply listen. The permission tree retains every requester and dependency path even when equivalent grants can be combined for the effective decision.

## Dynamic resource requests

A requirement may describe a capability without knowing the exact resource until runtime. For example, an HTTP library can declare that it may listen on a configured address. When application configuration selects `127.0.0.1:8087`, the runtime checks and requests that exact endpoint immediately before binding.

The runtime must not widen a concrete request automatically. Listening on `0.0.0.0:8087` is distinct from listening on `127.0.0.1:8087`. A request for port `0` must be checked as a bind request and report the selected ephemeral port after the operating system assigns it. Previously approved exact scopes may be remembered; a different endpoint requires a new decision unless a deliberately broader scope was approved.

## Runtime behavior when permission is missing

Missing consent does not block program startup. Authorization is checked at the protected operation boundary.

- If the operation returns a `Result` and the caller preserves it, permission denial is a typed, catchable error (for example, handled by `match`).
- In the already-defined ordinary value contexts where `Error` projects to `null`, denial yields `null`.
- The denied operation does not perform its protected side effect.
- The runtime emits a warning that the operation was attempted and denied, naming the capability/resource and requester provenance without exposing secret values.
- Avoid flooding the console: deduplicate repeated warnings for the same denied request within a run, while preserving useful audit information.
- A denial affects that operation; the rest of the program continues unless the program chooses to stop.

Warnings must say “attempted but denied,” not imply that the protected operation succeeded.

## Consent flows

### VS Code

When compiler/LSP metadata identifies a source operation that requires a capability, VS Code can offer an action such as **“Add requirement to `.zkinit`”**. It prepares the declaration and identifies whether the source is in the application or a dependency.

Adding a requirement is not granting consent. The editor must not silently convert a source edit into authorization. Concrete consent remains an explicit user action, or is obtained from the runtime prompt.

### Interactive CLI/runtime

If no applicable grant exists when an operation is reached, and the process has an interactive terminal, show a concise prompt with:

- application identity;
- exact capability and resource;
- package/requester and dependency path;
- explicit yes/no choice.

On approval, persist the exact, scoped consent in the application’s `.zkinit` configuration (creating the manifest if necessary) and allow the current operation. On rejection, deny the current operation, emit the warning, and continue with the typed error/`null` behavior above. Do not rewrite the manifest on rejection.

When there is no interactive terminal, do not hang or assume consent: deny the operation and report it. The application remains running if it handles the result.

## Provenance and dependency aggregation example

```text
my-app
└── lib-a
    └── lib-b
        └── requests network.listen at 127.0.0.1:8087
```

The effective request is for the concrete endpoint; the tree explains that `lib-b` requested it through `lib-a`. If another dependency requests a broader scope that contains this endpoint, show the broader request and its source clearly. Do not silently narrow the broader requester’s requirement or hide that it exists. Non-overlapping scopes remain separate.

## Environment API example

An operation such as `Env.get("APP_MODE")` is marked by API/compiler metadata as an environment-read capability. If the name is statically known, tooling can report the exact name. If it is dynamic, the runtime checks the actual name at the call boundary. The `requires` declaration describes the need; consent is still checked at runtime. The current implementation does not yet infer and propagate the complete `requires` graph.

## Decisions still open before a normative proposal

- Exact `.zkinit` grammar and where consent records live relative to package `requires` declarations.
- Whether a first-use approval is stored in `.zkinit`, a separate per-user approval store, or both. The agreed UX request is to persist approval and create/update `.zkinit` when needed; the storage/security binding still needs a precise design.
- How saved grants bind to canonical project location, user/device, dependency version/integrity, and manifest/lockfile changes; when reapproval is required.
- Scope syntax and root semantics for paths, hosts, ports/ranges, process arguments, environment names, and dynamic endpoints.
- Prompt behavior for concurrent requests, non-interactive execution, and persistence failures.
- How VS Code distinguishes a source requirement hint from a grant/consent action.
- Whether permissions are checked against only declared requirements or also support a runtime request for a narrower concrete scope within an API’s declared capability.

## Current repository status

The existing parser supports only part of the manifest and permission declarations. The feature-status catalog marks `requires` and build-time manifest permissions as not implemented. The present runtime/approval behavior and current specifications are not changed by this note. Reconcile this design with the existing `.zkinit`, runtime, compiler, and approval specifications when preparing the later general proposal.

## Finalized model

This discussion checkpoint is superseded by `openspec/changes/complete-zkinit-project-runtime/specs/zirk-permissions/spec.md` and the [permission handbook](docs/handbook/03-projects/07-runtime-permissions.md). There is one finite `requires` list for applications and packages; no permission, compile-permission, environment, or phase-specific manifest block remains. Requirements propagate in an internal requester/dependency-path tree and are not consent. Signed exact-resource consent is stored outside the repository. At first use, an interactive CLI shows project, operation, capability, resource, requester/path, then accepts `y/n`; yes stores that scope and permits the current operation, no denies only it, and no-TTY execution fails closed. Direct app consent can add a missing capability declaration; dependency requirements are not copied into the app manifest. `Global`/environment/network effects are checked at operation boundaries. Filesystem/process identifiers remain reserved until compiler-owned APIs exist.
