# zirk-packages

## Purpose

Defines how a Zirk project becomes an importable package, how package identity
and dependencies are resolved, and how published package artifacts remain
deterministic and verifiable for consumers.

## Requirements

### Requirement: Package projects have a declared logical identity

A project intended for reuse SHALL declare a package identity and package
project kind in its `.zkinit`. The logical package name SHALL be independent of
the package's physical checkout directory.

#### Scenario: Package root is discovered
- **WHEN** a package project contains a valid `.zkinit` and its configured source root contains the package entry module
- **THEN** the compiler assigns the declared logical package identity to the reachable modules
- **AND** changing the checkout directory does not change import resolution

### Requirement: Package roots define the public entry surface

A package SHALL have one public root module that publishes the modules and
declarations available to consumers. Files not reachable through that root or
another published module SHALL remain implementation details.

#### Scenario: Root re-exports a module
- **WHEN** a package root publishes `share mod terminal from "./terminal"`
- **THEN** a consumer can resolve `terminal` from the package name
- **AND** the consumer does not need to know the implementation file path

### Requirement: Package dependencies are declared by the project

An application or package SHALL declare external package dependencies through
`.zkinit`; source imports SHALL not silently acquire packages from arbitrary
filesystem locations.

#### Scenario: Declared dependency resolves
- **WHEN** a source file imports from an external package named `acme.cli` and the project manifest declares that dependency
- **THEN** resolution loads the dependency's verified public root
- **AND** the import is included in the source dependency graph

#### Scenario: Undeclared dependency is rejected
- **WHEN** a source file imports an external package absent from the project's declared dependencies
- **THEN** compilation emits a package-resolution diagnostic before type checking the importing body

### Requirement: Locked package resolution is deterministic

When `zirk.lock` is present, package resolution SHALL use its exact versions,
sources, and integrity hashes. A checksum or metadata mismatch SHALL reject the
package rather than substitute another artifact.

#### Scenario: Locked package is accepted
- **WHEN** the resolved package matches its lockfile version, source, and checksum
- **THEN** the compiler loads that package and records its logical identity

#### Scenario: Locked package is tampered with
- **WHEN** the downloaded or cached package does not match the lockfile checksum
- **THEN** compilation fails with a package-integrity diagnostic

### Requirement: Package transport is separate from source import syntax

Package acquisition MAY use a local path, registry, Git source, or vendored
artifact, but after resolution all package imports SHALL use the same unquoted
logical package path syntax.

#### Scenario: Different sources produce the same import
- **WHEN** `acme.cli` is resolved from a registry in one project and a local path in another
- **THEN** both source files use `import { terminal } from acme.cli;`
- **AND** the source code does not expose the transport mechanism

### Requirement: Package cycles are diagnosed with their complete path

Package dependency cycles SHALL be rejected before executable module loading,
with a diagnostic containing the complete package cycle. Acyclic packages MAY
contain source modules that are loaded once each according to module rules.

#### Scenario: Direct package cycle
- **WHEN** package `a` depends on `b` and package `b` depends on `a`
- **THEN** resolution rejects the graph and identifies `a -> b -> a`
