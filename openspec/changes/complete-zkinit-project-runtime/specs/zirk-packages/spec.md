## ADDED Requirements

### Requirement: Dependency declarations use logical package names

Each dependency SHALL be keyed by its stable logical package name. A version string SHALL select the configured default registry. An expanded dependency object SHALL use `from` as its source locator and MAY include `version` or `rev` as required by that source. Supported locators SHALL include `path:<relative-path>`, `git+<repository-url>`, `registry:<registry-name>`, and `vendored:<project-relative-directory>`. Source URLs and filesystem paths SHALL identify transport only and SHALL NOT become import names. Imports SHALL use the logical package name regardless of source.

#### Scenario: Registry dependency uses the concise form
- **WHEN** `.zkinit` declares `acme.http: "^2.1"`
- **THEN** the resolver looks up logical package `acme.http` in the configured default registry with that version requirement

#### Scenario: Local dependency keeps its package identity
- **WHEN** `.zkinit` declares `acme.shared: { from: "path:../shared"; }`
- **THEN** imports continue to use `acme.shared` and the resolver validates the target manifest's name and version

#### Scenario: Git dependency is pinned
- **WHEN** `.zkinit` selects a Git source with a human-readable revision
- **THEN** `zirk.lock` records the immutable resolved revision and integrity identity

#### Scenario: Named registry is selected explicitly
- **WHEN** `.zkinit` declares `from: "registry:company"` with a version requirement
- **THEN** resolution uses the named registry without changing the source import path

#### Scenario: Dependency import bypasses declared identity
- **WHEN** source attempts to import a URL or undeclared local dependency as a package name
- **THEN** resolution rejects the import and identifies the required logical dependency declaration

### Requirement: Lockfiles pin the complete resolved dependency graph

When a project has external dependencies, `zirk.lock` SHALL record each logical package name, selected version, source, immutable Git revision or artifact integrity value, and resolved package root. Locked builds SHALL not silently choose a different graph. A missing, stale, mismatched, or unverifiable lock entry SHALL produce a diagnostic or require an explicit lock update operation; it SHALL NOT silently substitute a package.

#### Scenario: Locked dependency matches its source
- **WHEN** the source, revision, version, package identity, and integrity value match `zirk.lock`
- **THEN** the compiler accepts the resolved package

#### Scenario: Lock metadata does not match the package
- **WHEN** a cached or downloaded package differs from its locked identity or integrity value
- **THEN** resolution fails before source execution

### Requirement: Development dependencies are isolated from consumer dependencies

`dev_dependencies` SHALL use the same package identity, source, version, and lock model as `dependencies`. They SHALL be available only to development commands explicitly designated by the CLI, including the project's test workflow, and SHALL NOT be inherited by downstream consumers or included in an ordinary application/package runtime graph unless a command explicitly requests the development graph.

#### Scenario: Test command loads a development dependency
- **WHEN** `zirk test` uses a package declared only in `dev_dependencies`
- **THEN** that package is available to the current project's test graph

#### Scenario: Consumer does not inherit a development dependency
- **WHEN** another project consumes a package that lists `acme.testkit` in `dev_dependencies`
- **THEN** `acme.testkit` is absent from the consumer's dependency graph

#### Scenario: Production graph omits development-only packages
- **WHEN** an application is built or run without a development command
- **THEN** development-only packages are not resolved into its ordinary runtime graph

### Requirement: Dependency lock changes are explicit CLI actions

`zirk install` SHALL resolve declared dependencies and write or verify `zirk.lock` without executing package or application code. `zirk update` SHALL be the explicit operation that selects newer versions permitted by manifest constraints. `zirk build`, `zirk check`, and `zirk run` SHALL use the locked graph and SHALL NOT silently rewrite versions or sources. When dependencies exist but the lock is missing or stale, the CLI SHALL report the required install/update action.

#### Scenario: Install resolves a project graph
- **WHEN** `zirk install` runs for a manifest with dependencies
- **THEN** it resolves the graph and writes the exact selected sources and integrity data to `zirk.lock`

#### Scenario: Run does not update the lock implicitly
- **WHEN** a locked dependency could be upgraded within its version constraint and `zirk run` executes
- **THEN** the currently locked version is used and `zirk.lock` remains unchanged

#### Scenario: Lock is missing
- **WHEN** a project declares remote or local dependencies but has no valid lockfile
- **THEN** compilation reports that `zirk install` is required before execution
