# Zirk package dependencies — design notes

**Status:** historical discussion checkpoint; the final contract is recorded below and in the linked OpenSpec/handbook sources.

**Date:** 2026-10-24

## Goal

Declare package dependencies by their stable logical package name, independently of the transport used to obtain their source. Source files import the logical package name; they do not import a URL or local installation path.

## Dependency identity and source

Each dependency entry is keyed by its logical package name. A normal dependency specifies a version requirement. The default source is the configured Zirk registry, so the common case stays short:

```text
dependencies {
    acme.http: "^2.1";
}
```

An expanded object selects an alternate source or named registry:

```text
dependencies {
    acme.http: "^2.1";

    acme.shared: {
        path: "../shared";
    }

    acme.crypto: {
        git: "https://code.example.com/acme/crypto";
        rev: "v1.4.2";
    }

    acme.private: {
        version: "^3.0";
        registry: "company";
    }
}
```

Source forms:

- **Default registry:** package name plus version requirement; use the project's configured registry.
- **Named registry:** version requirement plus an explicitly named registry, for private or alternate registries.
- **Local path:** relative path to a local package with its own `.zkinit`; useful for workspaces and local development.
- **Git:** repository URL plus an immutable revision/tag/commit for unpublished packages or controlled development.
- **Vendored source:** an explicitly vendored package directory may be supported as a distinct source when a project needs to carry package source with its tree.

The exact object keys and grammar remain proposals. `from` should describe the source locator, not replace package identity. A URL is transport metadata; it is not the name used by imports. Direct archive URLs, if supported, must be pinned by integrity hash and source identity in the lockfile.

## Version constraints and lockfile

`.zkinit` expresses the intended version range, such as `^2.1`. `zirk.lock` records the exact resolution used by a build, including logical name, selected version, source, revision/integrity hash, and package-root entry. A project without dependencies need not have a lockfile; once it has dependencies, builds should use the lockfile and must not silently resolve a different graph.

Local path dependencies use the package's declared logical name and version; the resolver should reject a mismatch between the dependency key/constraint and the target package manifest. Git dependencies use an immutable revision in the lockfile even when the manifest refers to a human-readable tag or branch.

Imports remain transport-independent:

```zirk
import { Client } from acme.http;
```

This import does not change whether `acme.http` came from a registry, a path, Git, or vendored source.

## Normal and development dependencies

If both groups are retained, they use the same package identity, source, and lockfile model:

```text
dependencies {
    acme.http: "^2.1";
}

dev_dependencies {
    acme.testkit: "^0.8";
}
```

`dependencies` are part of the package/application dependency graph consumed by builds and downstream users. `dev_dependencies` are available to development/test/tooling commands for the current project and are not inherited by a consumer of the package. This is a graph/inclusion distinction, not a second resolver or permission model.

The project must define which commands load each group. At minimum, `zirk build` and downstream package resolution use normal dependencies; `zirk test` and explicitly designated development tooling may use development dependencies. The exact command contract is still open.

## Environment conditions

Environment variables should not select where a package comes from. Registry credentials may be supplied through the user's environment/configuration, but dependency identity and resolution must remain reproducible and secrets must not be written to `.zkinit` or `zirk.lock`.

Conditional dependencies by platform/target could be considered separately in a future feature. They should use explicit, statically evaluable target conditions rather than arbitrary environment-variable interpolation. They are not required for the initial dependency model.

## Current implementation status

The CLI manifest model already represents a dependency as name, version constraint, and source (`local`, `registry`, `git`, or `vendored`). The current parser uses a flat declaration form rather than the proposed name-keyed shorthand/expanded object. Local and vendored graph traversal and lock metadata validation are implemented in part. Registry/Git acquisition, verified remote artifact hashes, publication, and a separate `dev_dependencies` graph remain pending per the feature-status catalog.

## Decisions still open before a normative proposal

- Exact manifest grammar for short version entries and expanded source objects.
- Default registry configuration and named registry identity/authentication.
- Version constraint grammar and how it applies to local and Git sources.
- Whether `vendored` is a separate source form or a local path mode.
- Whether Git revisions must be commit hashes in `.zkinit` or may be tags/branches pinned by `zirk.lock`.
- Exact lockfile format, update commands, integrity verification, and behavior when the lockfile is missing or stale.
- Whether `dev_dependencies` are included in the first package-manager release and which commands consume them.
- Whether/when target-conditional dependencies are supported.

## Finalized model

This discussion checkpoint is superseded by `openspec/changes/complete-zkinit-project-runtime/specs/zirk-packages/spec.md` and the [dependency handbook](docs/handbook/03-projects/11-dependencies.md). The canonical syntax uses the logical package name as the key, a version shorthand for the configured default registry, and an expanded `{ from: "..."; version: "..."; rev: "..."; }` object for explicit locators. `dev_dependencies` use the same identity/locator model but belong only to the current project's development graph. `zirk.lock` records selected source, version, immutable revision or integrity, and package root. Environment variables never select package identity or source. Registry/Git acquisition and publication remain partial; local/vendored graph validation is implemented.
