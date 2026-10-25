## Context

See `proposal.md` for motivation and scope. The current feature-status catalog reports `.zkinit` and multi-file projects as partial: project discovery, application/package identity, local and vendored graph traversal, and lock metadata validation exist, while registry/Git acquisition and package publication remain pending. The catalog marks the current `requires`/`during: build` manifest as not started. The live parser still represents separate environment/network permissions; the working tree also contains user-owned edits in the CLI project and approval code, which implementation must preserve and review rather than overwrite.

The design has to reconcile requirements that currently disagree: the project/package model uses one `entry` field for apps and packages, the type checker requires `fn main`, module loading checks static declarations but does not evaluate module bodies, and `.zkinit` currently receives the general source grammar in VS Code. The deltas in `specs/` define the intended external behavior.

## Goals / Non-Goals

**Goals:**

- Make one parsed, validated project model the source of truth for CLI, resolver, compiler, runtime, LSP, and extension behavior.
- Preserve deterministic source/package identity and initialization order across native builds.
- Make permission prompts operation-local and explain the exact dependency/requester path.
- Keep the first globals API small, statically typed, immutable by replacement, and reactive only to real value changes.
- Sequence delivery so every later layer consumes a stable earlier contract.

**Non-Goals:**

- Change `.zrk` import visibility, `share`, or the distinction between local and logical package imports.
- Add dynamic imports, environment-interpolated dependency sources, platform-conditional dependencies, or arbitrary package scripts.
- Add application-global state to reusable packages, broad mutable aliases, or a general-purpose event framework.
- Publish packages to a public registry or build an external registry service.
- Implement configuration semantic tokens before the parser/LSP can classify manifest fields reliably.

## Decisions

### 1. Freeze one manifest AST before changing compiler behavior

The `.zkinit` grammar remains Zirk-flavored declarative syntax and is parsed into a typed project model, not a bag of strings. Its first-version elements are `project`, repeated `source "<path>"` declarations, an application-only `build_targets` block, `dependencies`, `dev_dependencies`, `globals`, and one `requires` block. `project.name`, `project.version`, and `project.type` are required. `authors`, `description`, and `license` are optional. Applications use `entry`; packages use `root`. Preserve defaults already used by project discovery: `src/main.zrk` and `src/package.zrk`, respectively. Source roots default to `src`.

The `requires` block is a finite list of capability identifiers, without phase or per-environment subsections:

```text
requires {
    filesystem.read;
    network.listen;
}
```

Repeated `source "src";` declarations select project-relative source roots. An application `build_targets` block lists supported target identifiers, such as `"x86_64-linux"` and `"aarch64-macos"`. For application `zirk build`, explicit `--target` replaces the manifest list for that invocation; without it, the compiler builds every configured target, or the host target when the list is absent. Package builds use a CLI target or host detection. `zirk run` always selects the host target and does not run once per configured build target. A target list selects requested outputs; it does not restrict targets the compiler may support.

The combined forms are:

```text
project {
    name: "acme.api";
    version: "1.2.0";
    type: application;
    authors: ["Acme Engineering"];
    description: "HTTP API for Acme services.";
    license: "MIT";
    entry: "src/main.zrk";
}

source "src";

build_targets {
    "x86_64-linux";
    "aarch64-macos";
}

dependencies {
    acme.http: "^2.1";
    acme.shared: { from: "path:../shared"; }
    acme.crypto: { from: "git+https://code.example/acme/crypto"; rev: "v1.4.2"; }
    acme.private: { from: "registry:company"; version: "^3.0"; }
}

dev_dependencies {
    acme.testkit: "^0.8";
}

globals {
    request_count: UInt64;
    settings: {
        type: Settings;
        from: "src/config.zrk";
        readonly: true;
    }
    retry_limit: {
        type: UInt32;
        default: 0;
    }
}

requires {
    filesystem.read;
    network.listen;
}
```

```text
project {
    name: "acme.http";
    version: "2.1.0";
    type: package;
    authors: ["Acme Engineering"];
    description: "HTTP client and server primitives.";
    license: "Apache-2.0";
    root: "src/package.zrk";
}

source "src";

dependencies {
    acme.core: "^1.4";
}

dev_dependencies {
    acme.testkit: "^0.8";
}

requires {
    network.connect;
    network.listen;
}
```

The initial registry is exactly `filesystem.read`, `filesystem.write`, `network.connect`, `network.listen`, `network.udp.send`, `network.udp.listen`, `network.local.connect`, `network.local.listen`, `network.inspect_interfaces`, `environment.read`, `environment.list_names`, `secrets.read`, `process.execute`, and `process.shell`. Native/FFI capabilities remain outside the first release until a concrete API defines them. Unknown identifiers are manifest errors. A declaration describes capability needs; resource paths, hosts, ports, executable arguments, and environment names are checked at the operation boundary.

Alternatives considered: retain separate `permissions` and `requires` forms, or keep build/runtime phases. Both repeat the same concept in application configuration and make dependency aggregation harder to explain. The accepted model uses one declaration vocabulary and no phase dimension; consent remains a separate internal decision so that adding a declaration cannot silently authorize code.

### 2. Keep dependencies keyed by logical package identity

The manifest key is the stable package name used by source imports. A version string is the concise default-registry form; expanded forms identify a source with `from` and any required revision/version data:

```text
dependencies {
    acme.http: "^2.1";
    acme.shared: { from: "path:../shared"; }
    acme.crypto: { from: "git+https://code.example/acme/crypto"; rev: "v1.4.2"; }
    acme.private: { from: "registry:company"; version: "^3.0"; }
}

dev_dependencies {
    acme.testkit: "^0.8";
}
```

`from` is a transport locator, never an import identity. Supported locators are the configured/default or named registry, project-relative path, Git repository, and project-vendored directory. URLs must use an explicit supported source form; a direct archive must also carry an integrity identity. Credentials come from user/environment configuration and never enter `.zkinit` or `zirk.lock`.

`zirk.lock` pins logical name, selected version, source locator, immutable Git revision or artifact hash, and package root. `zirk install` resolves and writes the lock; ordinary build/run uses the lock and never silently changes it; `zirk update` deliberately refreshes eligible versions. `dependencies` form the normal build/run graph. `dev_dependencies` join only commands designated as development/test commands and are never inherited by consumers. Target-conditional dependencies are deferred.

Alternatives considered: importing from URLs or local paths, or selecting sources through arbitrary environment variables. Those approaches make source imports machine-specific and weaken reproducibility. Keeping one logical name across source transports preserves stable imports and lock identities.

### 3. Select a file at the CLI boundary; do not require source `main`

`zirk run <path>` selects that source file. If it is under a project root, the closest containing `.zkinit` supplies dependencies, lock state, globals, and capability provenance; the path overrides only the configured application entry. Without a path, `zirk run` discovers the nearest project manifest and requires `type: application`, then executes its `entry`. A package with no explicit run path and a pathless invocation without a manifest produce targeted diagnostics. A file run outside a project stays a standalone entry with local/standard import rules.

The source entry is a sequence of top-level statements. Function/type declarations define members but are not invoked automatically. `fn main` becomes an ordinary function. Native code retains an internal ABI `main` solely to initialize the runtime, dispatch to Zirk, and return the process status.

Imports remain static and must precede executable statements. The compiler loads, parses, resolves, and checks the complete reachable graph before executing any user code. Runtime evaluation is deterministic depth-first in import source order: initialize each dependency first, then its importer, once per canonical module identity, then execute the selected application entry. Re-exporting a module does not repeat it. Declaration-only source cycles remain valid; a strongly connected component containing executable top-level initialization is rejected before execution rather than exposing partially initialized state.

Alternative considered: keep `main` as an implicit function hook. That would make the manifest's `entry` insufficient to explain startup and would leave two competing entry selectors. The CLI-selected file is the only source-level startup selector.

### 4. Initialize typed globals between project validation and module execution

Only an application owns the global symbol table. Shorthand declarations infer their default from the declared type. Expanded declarations require `type` and may include `default`, `from`, or `readonly`; `readonly` defaults to false and affects only `Global.set`. `from` resolves a custom type relative to the project root, loads that file and any type dependencies needed to check it, and treats files reached only for the manifest type as type-only so their file-scope statements do not run. The direct `from` file must not contain executable file-scope statements. Without `from`, normal package/module resolution applies.

The semantic checker consumes the manifest table before checking source calls. `Global.get("key")`, `Global.set("key", value)`, and `Global.watch("key")` require literal keys initially. Their `T` is inferred from `.zkinit`; an explicit generic cannot override it. Unknown keys, mismatched values, dynamic keys, unresolvable types/defaults, and writes to read-only slots are diagnostics. A custom global is default-constructed through one uniquely eligible constructor: each parameter uses its declared default or, otherwise, its parameter type's default. The constructor body runs once using ordinary language semantics; constructor errors become startup initialization failures. The selected application entry's top-level body is never run to derive a manifest default.

Runtime state stores each global in a GC-traced mutex cell rooted for the process lifetime. Generated initialization allocates the cell after runtime startup and before imported module bodies. `get` copies the stored value into a GC-rooted staging slot while holding the cell lock, releases the lock, then deep-clones the value for the caller. `set` deep-clones the replacement before acquiring the lock and publishes it as one replacement while holding the lock. This keeps user mutation and clone allocation outside the critical section. The initial implementation accepts only types for which the compiler can prove structural `Clone` semantics; unsupported mutable containers and identity-bearing fields produce a diagnostic rather than exposing an alias.

Structural value equality is deterministic and compares contents, not identity. Unsupported equality types are rejected for `watch`. `Global.watch<T>(key)` returns `GlobalSubscription<T>` with `snapshot(): GlobalSnapshot<T>`, `next(): Result<GlobalChange<T>?, GlobalWatchError>`, and `close(): Void`. Watch emits no initial event and reports `{ previous, current, revision }` only for unequal replacements. Registration returns an atomic initial snapshot/revision, updates have monotonically increasing per-global revisions, observers run outside locks, and bounded queues return one terminal `GlobalWatchError.Lagged` rather than silently dropping events. After lag, the caller obtains a fresh snapshot from a new subscription.

The native runtime uses a hub keyed by each rooted global-cell handle. Generated registration and publication hold that cell's mutex while calling the hub, so the captured snapshot/revision and event order share one serialization boundary. The hub owns per-global revisions and a bounded typed-channel handle per subscription; it serializes nonblocking channel send/receive/close operations under its own mutex, roots each channel until explicit close, and releases the root on close or global reinitialization. On overflow it closes that channel, drains already accepted events, reports `Lagged` exactly once, and then reports the subscription as closed. `next()` is nonblocking and returns `null` when the queue is empty. The C ABI is `zirk_rt_global_watch_reset`, `zirk_rt_global_watch_subscribe`, `zirk_rt_global_watch_publish`, `zirk_rt_global_watch_next`, and `zirk_rt_global_watch_close`; compiler lowering owns construction of typed snapshots/events and passes the change decision into publication. A local binding owns a subscription until its lexical block exits; the compiler closes it through the same runtime path on normal block exit, return, break, continue, exception propagation, and cancellation. Returning a handle transfers it to the caller, whose binding then owns its lifetime. Aliasing does not extend the owning block's lifetime, so an alias used after that block reports `GlobalWatchError.Closed`. This gives the initial surface deterministic cleanup without introducing a general ownership or lifetime system.

Alternative considered: identity-based equality and in-place updates. Identity would fire for equal reconstructed objects and miss the Redux-style state transition contract; mutable aliases could bypass notifications. Value snapshots make the event rule predictable at the cost of requiring supported value-copy/equality semantics.

### 5. Separate declared capability needs from saved user consent

The compiler tags privileged standard APIs with capability metadata and propagates it through callables, closures, and dependency edges. The requirement tree is assembled from the complete locked graph and retains every requester/version/path; equivalent resource approvals may share an effective decision without hiding provenance. A package cannot grant itself authority.

The capability-to-API map is closed over the compiler-owned standard bindings. Every future privileged binding must be added to this table and checked at the operation boundary before its side effect:

| Standard API surface | Capability | Runtime enforcement boundary |
| --- | --- | --- |
| `Environment.get`, `require`, and `contains` | `environment.read` | Environment lookup leaf, immediately before reading the named variable. |
| `Environment.list_names` | `environment.list_names` | Name-enumeration leaf, immediately before enumerating the environment. |
| `Environment.get_secret` | `secrets.read` | Secret-value leaf, immediately before reading the named secret. |
| DNS resolution used for outbound access; TCP, TLS, HTTP client, and WebSocket client connections, redirects, and reconnects | `network.connect` | Concrete origin/address check in the shared network policy before each connection attempt. |
| TCP, TLS, and HTTP server bind/listen operations | `network.listen` | Concrete address-and-port check immediately before the operating-system bind/listen call. |
| UDP connect, send, send-to, broadcast, and multicast operations | `network.udp.send` | Concrete destination check immediately before datagram transmission; connection setup does not widen it. |
| UDP bind and receive operations | `network.udp.listen` | Concrete address-and-port check before socket bind; later receives remain owned by that authorized socket. |
| Local socket connect operations | `network.local.connect` | Exact local endpoint check immediately before connecting. |
| Local socket bind/listen operations | `network.local.listen` | Exact local endpoint check immediately before binding. |
| Network interface snapshot/enumeration | `network.inspect_interfaces` | Interface-inspection leaf immediately before enumerating interfaces. |

The current environment implementation leaves fail closed at its host leaves, while the network implementation verifies the launcher grant and enforces normalized scopes through the shared network policy. No compiler-owned filesystem or process-execution standard API is currently exposed; `filesystem.read`, `filesystem.write`, `process.execute`, and `process.shell` remain reserved capability identifiers until their APIs and host check points are implemented. Network stream read/write/close operations inherit the authorization of the socket that created the stream and do not request a broader capability.

The `requires` list is not consent. A per-user trusted approval store persists exact resource approvals and binds them to project identity/location, user/device, lock/requester identities, and dependency paths. This avoids treating a checked-in manifest edit as a user's yes. On the first interactive denial-free request, the CLI presents application identity, capability, concrete resource, requester, and path, then asks yes/no. A yes persists the exact grant, ensures an application `.zkinit` exists, and records the capability in the owning application's manifest only when that requirement is direct and missing; it does not copy every transitive library requirement. For a standalone file, the generated manifest uses a stable `local.<normalized-directory>` name, version `0.1.0`, the selected file as `entry`, and source root `.`. A no writes nothing. A non-interactive run denies without hanging. Rejection returns through the API's `Result`; if the caller uses ordinary contextual projection, it becomes `null`. The denied side effect does not occur, and a deduplicated warning says the operation was attempted and denied.

Dynamic checks use the resource known at the call boundary: canonical filesystem path, exact network origin/listener address and port, normalized process executable/arguments, or environment/secret name. Redirects, DNS answers, reconnects, proxy hops, and binds are rechecked. Listening on a configured dynamic port asks for that exact endpoint; no implicit all-host/all-port grant exists. A request to bind port `0` is checked before binding as a request for an OS-assigned ephemeral port on the specified address; after success, the runtime reports the actual endpoint and does not reinterpret consent as an unrelated port range. There is no separate build/test/runtime permissions syntax.

Alternative considered: fail compilation or stop application startup if any library declares a capability. That punishes unused paths and prevents applications from handling denied operations. Runtime checks keep denial local to the exact operation while retaining fail-closed behavior before the protected side effect.

### 6. Make `.zkinit` editor support follow the canonical grammar

Keep the `zirk-init` language ID and icon, but add configuration-specific TextMate scopes instead of applying every `.zrk` rule unchanged. Scope `project`, `source`, `build_targets`, dependencies, globals, and requires as structural sections. Map the agreed palette by role: sections violet `#8E6CFF`, keys warm yellow `#F7C65C`, built-in types teal `#32D4C6`, user types green `#34D399`, enum-like values orange `#F4A261`, strings/versions/target identifiers pink `#FF4FA3`, numbers blue `#60A5FA`, and comments muted gray `#A3A3AF` italic. The active theme remains authoritative; the palette is role mapping and must remain readable on light and dark themes.

LSP config parsing, diagnostics, global type inference, capability hints, and version-aware code actions consume the same compiler project model. A source action may add the smallest missing capability declaration; it never records runtime consent. Dependency provenance is shown explicitly; an external dependency manifest is not edited by an action in the consuming project.

Alternative considered: use semantic tokens to classify configuration fields immediately. TextMate scopes are deterministic before the LSP understands the manifest; semantic tokens arrive only after compiler classifications are reliable.

### 7. Initialize projects with the canonical manifest

`zirk init` is the last product feature before documentation and website synchronization. It operates in the current directory and follows the `npm init` interaction pattern: ask one field at a time for application/package type, project name, version, the type-specific `entry` or `root`, optional authors/description/license, source roots, and application build targets. Every prompt shows its default; Enter accepts it, supplied input replaces it, and optional metadata/build targets can be left unset. The package choice uses `src/package.zrk` as its default root and omits application-only `entry` and `build_targets`.

`zirk init -y` runs without prompts and accepts the documented default for every field, creating an application manifest with `project.name` derived from the normalized current-directory name (falling back to `zirk-project`), `project.version: "0.1.0"`, `project.type: application`, `project.entry: "src/main.zrk"`, and source root `src`. Defaults for optional metadata and application build targets are unset, so those fields are omitted and target selection uses host detection. Dependencies, development dependencies, globals, and `requires` start empty; no consent record is inferred or created. The command writes only `.zkinit`, not source files.

Both modes build the same typed project model, validate it, format it with the canonical formatter, and atomically create the manifest. They refuse to overwrite an existing `.zkinit`. Canceling, invalid answers, validation errors, or a non-interactive invocation without `-y` leave no partial file. The initializer does not install dependencies, generate source files, or grant capabilities.

### 8. Implement in dependency order

1. **Contract and migration baseline** — confirm examples, capability identifiers, dependency locator forms, entry/root/source-root defaults, target precedence, legacy-syntax diagnostics, and approval persistence rules. Capture the exact behavior expected from the uncommitted project/approval edits before touching implementation.
2. **Manifest parser and project model** — parse project metadata, source roots, build targets, and the five capability/dependency/global sections into typed nodes with spans; validate project kind, required/optional fields, duplicate keys, defaults, and legacy permission syntax. Add a canonical formatter round-trip before wiring runtime behavior.
3. **Dependency resolver and lock** — support default/named registry, path, Git, and vendored sources, exact lock validation, stable logical package imports, and the isolated dev graph. Complete this before permission aggregation because the resolver supplies requester identity/provenance.
4. **Entry selection and module execution** — add explicit/default `zirk run` selection, standalone/project context resolution, top-level body typing, no-main checks, deterministic module execution, cycle diagnostics, unhandled startup failure, and internal native wrapper dispatch.
5. **Global symbol/type layer** — resolve custom types/defaults, seed the project symbol table, infer the three `Global` calls, diagnose read-only/type/key failures, and initialize values before module execution.
6. **Permission effect tree and runtime consent** — infer API effects, merge dependency provenance without dropping duplicates, implement per-operation exact-scope checks and typed denial/null projection, prompt and store yes/no decisions, create/update `.zkinit` as agreed, and verify non-interactive fail-closed behavior.
7. **LSP and VS Code** — add manifest overlays and diagnostics, palette grammar, source code actions, requester provenance, and no-command-on-manifest behavior. This phase depends on stable parser and compiler diagnostics.
8. **Project initializer (last product feature before synchronization)** — add interactive `zirk init` and non-interactive `zirk init -y` using the canonical parser, formatter, defaults, and atomic no-overwrite behavior. Complete and verify this feature before synchronizing documentation or the website.
9. **Documentation, status, and release synchronization** — update normative specs/handbook/examples, Feature Status, Current Limitations, pipeline, roadmap, and migration notes to reflect verified `zirk init` behavior alongside the rest of the project contract. After the Zirk documentation commit, sync and review `../zirk-lang-site`; status changes require site-owned catalog review and an explicit audit date.
10. **Final integration gate** — validate each phase against its specs and report implemented behavior, evidence, limitations, documentation/site revisions, and remaining work without presenting planned behavior as delivered.

## Risks / Trade-offs

- **[Top-level initialization introduces import side effects]** → Require static imports first, compile the complete graph before execution, run each module once, document initialization order, and reject executable initialization cycles.
- **[A changed `fn main` convention breaks existing programs]** → Ship a migration diagnostic and guide showing the old function body moved to top level or explicitly called as `main()`; do not silently auto-invoke it under the new contract.
- **[Runtime manifest writes can surprise users or dirty a checkout]** → Write only after an explicit yes, use the canonical formatter and atomic file replacement, create only a minimal manifest when missing, preserve unrelated formatting/data, and never edit on no.
- **[A requirement edit could be mistaken for consent]** → Keep consent in the trusted user store, bind it to exact project/dependency fingerprints, and require fresh approval on changed identity/scope/requester.
- **[Dynamic scopes can be broader than an API's intended resource]** → Check normalized concrete resources immediately before side effects; distinguish connect/listen, TCP/UDP, read/write, and shell/process operations.
- **[Watch value copying/equality may exclude types]** → Define a compiler-checked value/equality contract, report unsupported types, bound queues, and expose lag rather than silently losing events.
- **[Large scope delays usable delivery]** → Implement and review in the numbered gates; each later phase depends on a validated earlier artifact, and no phase should be reported as delivered until its runtime behavior and status evidence are verified.
- **[Project initialization overwrites user configuration or invents consent]** → Refuse to overwrite `.zkinit`, validate before an atomic create, keep capability requirements empty by default, and never create consent records from `zirk init`.
- **[Website status becomes stale]** → Treat website synchronization as a release gate after the authoritative docs are committed; review the website's status catalog rather than deriving its public claims from source or task completion.

## Migration Plan

1. Add new manifest forms and diagnostics without deleting the old parser path until the migration tool can read both forms.
2. Provide `zirk migrate` preview/apply for legacy `.zkinit`: normalize project metadata, preserve source roots and application build targets, convert a package's legacy `entry` to `root`, convert dependency declarations to logical-name keyed shorthand/`from` objects, and create missing optional metadata only when supplied by the user. Never infer a package version or source hash that is not present.
3. Convert legacy permission declarations to `requires` capability identifiers, drop phase labels, and treat every old scope as unapproved. Show a diff; the migration never creates consent. Existing approvals must be reconsidered under the new exact-resource binding.
4. Migrate old `fn main` source by moving its body into top-level statements or by adding an explicit top-level `main()` call. The migration tool may offer a source diff but SHALL NOT apply it without explicit user action.
5. Resolve the dependency graph into `zirk.lock` before a build/run that uses remote sources. `zirk install` is the only implicit lock creation path; `run` and `build` never silently update selected versions.
6. Keep the old syntax reader for one transition window if needed by repository examples, but emit a deprecation/migration diagnostic and never convert old manifest grants into saved consent. Remove old parsing only after repository examples/docs are migrated and feature-status evidence confirms the new path.

Rollback is version-based and must not discard user files: a previous compiler may read an unconverted legacy manifest; it may not understand the new form. The migration command therefore retains a backup beside the manifest and reports that rollback requires restoring the backup or using a compiler that supports the new schema. Consent records remain in the user-owned approval store and can be revoked independently.

Registry credentials reuse the existing user/environment configuration; this change adds no credential-helper command. The subscription method contract is fixed above and does not depend on adding a language-level stream iteration syntax.
