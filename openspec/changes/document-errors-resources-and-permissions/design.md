## Context

Zirk's existing documents name `Result`, exceptions, `fatalError`, `Resource<E>`, `match with`, and manifest permissions, but leave critical composition rules open. The accepted authorial rounds now define a single model spanning source syntax, type/effect checking, deterministic cleanup, package requirements, runtime enforcement, and developer consent. The design must preserve the previously accepted callable, projection-copy, strict-alias, and structured-concurrency foundations while remaining documentation-only in this change.

## Goals / Non-Goals

**Goals:**

- Make expected, exceptional, implicit-runtime, and fatal failures disjoint and implementable.
- Make resource responsibility statically visible and cleanup deterministic on every exit.
- Make package authority least-privilege, user-approved, auditable, incremental, and resistant to repository tampering.
- Keep ordinary function signatures readable while retaining compiler-internal exception and permission effects.
- Give contributors canonical documentation, API catalogs, examples, and phase boundaries.

**Non-Goals:**

- Implement compiler, runtime, package-manager, OS keychain, or sandbox behavior in this change.
- Finalize memory management, `unsafe`, task scheduling, or decorator transformation semantics beyond their interaction boundaries.
- Permit automatic broad permission grants, deployed runtime prompts, public ownership syntax, or implicit conversion between `Result` and exceptions.

## Decisions

### D1 — Four failure channels remain distinct

`Result<T,E>` represents expected failure and MUST be consumed or explicitly discarded. Explicit exceptional behavior is declared with `throws`; safety checks raise typed implicit `RuntimeError` values without polluting every signature; `fatalError` never returns. This separation preserves Go-like visible operational failure without sacrificing typed cross-layer exceptions or safe runtime checks.

### D2 — Explicit exceptions are checked; implicit runtime exceptions are catchable

Every source-level `throw X(...)` must be handled or included in `throws X`; public APIs spell the complete explicit set. Built-in bounds, division, overflow, cast, state, allocation, and invalidation failures belong to a documented `RuntimeError` hierarchy and remain catchable without mandatory declaration. `Fn` records explicit `throws` only, and callable compatibility is contravariant in allowed exceptional effects.

### D3 — Errors have immutable identity and complete provenance

`Error` is a nominal requirement; `Throwable` adds stack-trace behavior and `RuntimeError` classifies implicit safety failures. Thrown objects are deeply immutable reference identities. `throw;` preserves the current exception and trace; wrapping creates a new throwable with `cause`. Concurrent cleanup failures attach to `suppressed` without replacing the primary failure. Stack traces materialize lazily and exclude locals/secrets.

### D4 — Resource responsibility is affine without public ownership syntax

`Resource<E from Error>` closes through a typed `Result`. `match with` owns successful acquisitions and closes exactly once on every exit. Transfer is explicit, invalidates the original binding, and is available only through the transfer contract. Resources do not implement ordinary `Clone`; duplication is a distinct fallible type API. The checker rejects provable leaks and illegal escapes, while runtime state catches dynamic closed/transferred misuse.

### D5 — Body and cleanup failures are never collapsed

Successful-body close failure becomes the block failure. An exception remains primary and receives cleanup failures as suppressed errors. A `Result.Error` combined with close failure uses `ResourceFailure<BodyError,CloseError>` with `Body`, `Close`, and `BodyAndClose`. Multiple acquisition proceeds left-to-right and unwinds right-to-left.

### D6 — Public permission syntax has two authorities

Libraries use `requires`; applications use `permissions`. A grant carries `during: build | runtime | both`, replacing a separate `compile_permissions` block without merging build-machine and deployed authority. Libraries can request but never grant. Permission needs propagate automatically through the call graph and callable metadata but are not written in `Fn` syntax.

### D7 — Declaration never equals consent

`init.zrk` declares requested authority. A separate signed approval record binds the exact permission/dependency fingerprint to project name, canonical location, OS user, and device. Moving or renaming a project invalidates approval. Permission widening, requester addition, requester update, phase expansion, or dependency-path change requires reapproval. Reduction does not. A package cannot forge approval by editing repository files.

### D8 — Permission validation is incremental and fail-closed

Zirk hashes the manifest, requester subgraphs, lockfile integrity, phases, and signed policy. Unchanged fingerprints take a constant-time fast path. Changes recompute only affected graph segments. Missing, deleted, corrupt, moved, or unverifiable approval state causes analysis/reapproval rather than authority. Build code cannot access approval storage through public APIs.

### D9 — Interactive convenience remains explicit

In an interactive trusted CLI, Zirk may show the exact permission, phase, call path, requesting dependency, and manifest diff, then offer allow-once, approve-exact-set, or deny. Acceptance edits `init.zrk` structurally and records signed consent. Dynamic scopes require manual patterns; `all` requires typing the project name. CI uses an explicit protected policy. Deployed programs never prompt or self-edit and receive typed permission denial.

### D10 — Scope enforcement is operation-specific

Filesystem checks canonical paths and symlink escape; network checks scheme, host, port, redirect, DNS resolution, and reconnect; environment access distinguishes secrets and returns `SecretString`; process execution can constrain canonical executable and arguments; shell execution is a separate high-risk grant. `Environment` and exact alias `Env` provide static accessors.

## Risks / Trade-offs

- **Signed local approval can be deleted by the user or external tooling** → deletion grants nothing and forces reapproval; tampering invalidates the signature.
- **Canonical paths make moving a project inconvenient** → this is intentional traceability; the prompt displays old/new location and requests fresh consent.
- **Dependency updates prompt even when textual permissions match** → code receiving authority changed, so reapproval is required; incremental analysis keeps unchanged builds fast.
- **Implicit runtime exceptions weaken complete effect enumeration** → their hierarchy and operation tables are documented, catchable, and compiler-known; only the propagation obligation is omitted.
- **Resource failure unions can enlarge types** → explicit `ResourceFailure` prevents lost cleanup errors and helper APIs can map it at boundaries.
- **Permission metadata outside `Fn` is less visible in annotations** → documentation, compiler diagnostics, call-chain explanations, and retained callable metadata expose it without destabilizing callable type compatibility.
