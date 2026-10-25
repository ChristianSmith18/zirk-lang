## Purpose

Defines the canonical `.zkinit` project identity and the rule for selecting and executing an application source file. It keeps application startup separate from package publication while allowing an explicit source path to run inside its project context.

## Requirements

### Requirement: Applications and packages declare one project identity

Every project manifest SHALL declare `project.name`, `project.version`, and `project.type`, where `type` is `application` or `package`. `authors`, `description`, and `license` SHALL be optional metadata. `authors` SHALL accept a list of names, and `license` SHALL accept an SPDX identifier or expression. A manifest SHALL reject fields that are invalid for its project type.

#### Scenario: Application metadata is optional
- **WHEN** an application declares its identity without `authors`, `description`, or `license`
- **THEN** the manifest remains valid
- **AND** tooling reports the project identity without fabricating omitted metadata

#### Scenario: License metadata is validated
- **WHEN** a project supplies a malformed license expression
- **THEN** manifest validation emits a field-specific diagnostic

### Requirement: Projects configure source roots and build targets

A project MAY declare one or more project-relative source roots using repeated `source "<path>"` declarations; when omitted, the source root SHALL default to `src`. An application MAY declare a `build_targets` block containing a non-empty, duplicate-free list of supported target identifiers. A package SHALL NOT declare `build_targets`. Source roots SHALL remain within the canonical project root. `build_targets` SHALL select build outputs but SHALL NOT restrict the set of targets the compiler can attempt.

For an application, `zirk build` with an explicit CLI `--target` SHALL override the manifest list for that invocation. Without a CLI target, the compiler SHALL build each manifest target when the list is present, or use host detection when it is absent. A package build SHALL use an explicit CLI target or host detection. `zirk run` SHALL select the host target and SHALL NOT execute every target listed for builds.

#### Scenario: Source roots use the default
- **WHEN** a project does not declare any source root
- **THEN** project loading uses `src`

#### Scenario: Project declares multiple source roots
- **WHEN** `.zkinit` declares `source "src"` and `source "generated"`
- **THEN** both roots participate in the project source graph in declaration order

#### Scenario: Build uses manifest targets
- **WHEN** `zirk build` runs without `--target` and `.zkinit` lists two supported targets
- **THEN** the build produces artifacts for both targets

#### Scenario: CLI target overrides manifest targets
- **WHEN** `zirk build --target x86_64-linux` runs while the manifest lists other targets
- **THEN** only the explicitly selected target is used for that invocation

#### Scenario: Invalid or duplicate target is declared
- **WHEN** `build_targets` contains an unsupported or duplicate target identifier
- **THEN** manifest validation reports the invalid entry before code generation

#### Scenario: Package declares build targets
- **WHEN** a package manifest contains `build_targets`
- **THEN** manifest validation rejects the block as application-only

### Requirement: Applications select an entry and packages select a root

An application SHALL use `project.entry` as the source file whose top-level body starts that application. A package SHALL use `project.root` as its importable public root. `entry` SHALL be invalid for packages and `root` SHALL be invalid for applications. If omitted, the application entry SHALL default to `src/main.zrk` and the package root SHALL default to `src/package.zrk`. Both paths SHALL resolve within the project's canonical root.

#### Scenario: Application uses its configured entry
- **WHEN** an application manifest sets `entry: "src/server.zrk"`
- **THEN** project execution selects that file as the application entry

#### Scenario: Package root is not executable entry metadata
- **WHEN** a package manifest sets `root: "src/package.zrk"`
- **THEN** the path is used as its importable root
- **AND** the package is not treated as an application entry

#### Scenario: Entry escapes the project root
- **WHEN** a manifest entry or package root resolves outside the canonical project root
- **THEN** project loading fails with a path-containment diagnostic before reading the target

### Requirement: Explicit and manifest-selected run entries preserve project context

`zirk run <path>` SHALL execute the supplied source file. If that file is inside a project, the nearest applicable `.zkinit`, lockfile, dependencies, globals, and requirements SHALL remain active; the supplied path SHALL override only the configured application entry for that invocation. If no project manifest applies, the CLI SHALL treat the file as a standalone entry and apply standalone import rules.

`zirk run` without a path SHALL discover the applicable `.zkinit`, require `project.type: application`, and execute its configured or default entry. It SHALL report a targeted diagnostic when no project manifest exists or the discovered project is a package.

#### Scenario: Explicit file overrides the configured entry
- **WHEN** `zirk run src/dev.zrk` is invoked inside an application whose manifest selects `src/main.zrk`
- **THEN** `src/dev.zrk` executes
- **AND** the application's manifest, lockfile, globals, dependencies, and requirements still apply

#### Scenario: Run discovers the application entry
- **WHEN** `zirk run` is invoked from an application project
- **THEN** the CLI executes the manifest-selected entry

#### Scenario: Run without an entry in a package project
- **WHEN** `zirk run` is invoked without a source path from a package project
- **THEN** the CLI reports that a package has no application entry

#### Scenario: Standalone file has no project manifest
- **WHEN** `zirk run path/to/tool.zrk` is invoked without an applicable `.zkinit`
- **THEN** the file is executed as a standalone entry using the standalone module rules

### Requirement: The selected source file is the program entry

The selected application entry SHALL execute its top-level executable statements once, in source order, after runtime and application-global initialization and after its imported modules have initialized. Function and type declarations SHALL define callable or type members and SHALL NOT execute their bodies merely by appearing in the entry file. A source-level `fn main` MAY exist as an ordinary function but SHALL NOT be required or invoked implicitly.

#### Scenario: Top-level statements start the application
- **WHEN** an application entry contains two executable top-level statements in source order
- **THEN** both execute in that order as the application body

#### Scenario: A declared function is not implicitly invoked
- **WHEN** an entry declares `fn main()` but does not call it
- **THEN** the function body does not run solely because its name is `main`

#### Scenario: Entry completes normally
- **WHEN** all imported module initialization and entry top-level statements complete
- **THEN** the runtime performs its normal resource and task shutdown before returning the process status

### Requirement: Manifest errors identify the affected field

Malformed project metadata, incompatible project-type fields, invalid source paths, unresolved entry/root files, and unsupported manifest versions SHALL produce structured compiler or CLI diagnostics that identify the field and source location. Project loading SHALL finish before program code executes.

#### Scenario: Package manifest contains an application-only field
- **WHEN** a package manifest declares `entry`
- **THEN** validation identifies `project.entry` as invalid for a package

#### Scenario: Missing selected source file
- **WHEN** the resolved application entry or package root does not exist
- **THEN** project loading reports the resolved missing path before runtime startup

### Requirement: Legacy project manifests can be migrated explicitly

`zirk migrate` SHALL provide a preview and an explicit apply mode for converting supported legacy `.zkinit` forms to the canonical project model. Migration SHALL preserve project identity and supported dependency intent, convert a package's legacy `entry` to `root`, map legacy capability declarations to `requires`, and show all changes before writing. Legacy permission grants SHALL become unapproved requirements; migration SHALL NOT create or widen saved consent. Migration SHALL preserve a recoverable copy of the original manifest and SHALL NOT rewrite source files without a separate explicit source-migration choice.

#### Scenario: Preview a legacy application manifest
- **WHEN** `zirk migrate` is run in preview mode on a supported old manifest
- **THEN** it displays the canonical `.zkinit` diff without changing files

#### Scenario: Apply project migration
- **WHEN** the user explicitly applies a reviewed manifest migration
- **THEN** Zirk writes the new manifest atomically and preserves a recoverable original

#### Scenario: Legacy grant is migrated
- **WHEN** an old manifest contains an application permission scope
- **THEN** the new manifest records only the corresponding capability requirement and the runtime still requires fresh consent for the concrete resource

#### Scenario: Old main function requires source migration
- **WHEN** a legacy source entry defines `fn main` but has no top-level invocation
- **THEN** migration reports the needed source change and does not silently invoke or rewrite the function
